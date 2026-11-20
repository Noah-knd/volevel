use std::collections::HashMap;

use crate::config::{AppConfig, ControlMode, DuckRole};

#[derive(Debug, Clone)]
pub struct SessionInput {
    pub key: String,
    pub peak: f32,
    pub current_volume: f32,
    pub muted: bool,
}

#[derive(Debug, Clone)]
pub struct SessionDecision {
    pub key: String,
    pub target_volume: Option<f32>,
    pub effective_volume: f32,
    pub display_peak: f32,
    pub gain_reduction: f32,
    pub ducking_active: bool,
}

#[derive(Debug, Clone)]
struct SessionRuntime {
    mode_gain: f32,
    smoothed_peak: f32,
    last_applied: Option<f32>,
    suppress_sync_ticks: u8,
}

#[derive(Debug, Default)]
pub struct ControlEngine {
    runtimes: HashMap<String, SessionRuntime>,
    duck_hold_ms: f32,
    duck_factor: f32,
    duck_initialized: bool,
    was_bypassed: bool,
}

impl ControlEngine {
    pub fn new() -> Self {
        Self {
            runtimes: HashMap::new(),
            duck_hold_ms: 0.0,
            duck_factor: 1.0,
            duck_initialized: true,
            was_bypassed: false,
        }
    }

    pub fn step(
        &mut self,
        inputs: &[SessionInput],
        config: &mut AppConfig,
        dt_ms: f32,
    ) -> Vec<SessionDecision> {
        if !self.duck_initialized {
            self.duck_factor = 1.0;
            self.duck_initialized = true;
        }
        let dt = dt_ms.clamp(1.0, 200.0);
        let bypass = config.bypass;

        if bypass && !self.was_bypassed {
            for rt in self.runtimes.values_mut() {
                rt.suppress_sync_ticks = 8;
            }
        }
        self.was_bypassed = bypass;

        let mut trigger_active = false;
        if !bypass {
            for s in inputs {
                if !s.muted {
                    if let Some(rule) = config.apps.get(&s.key) {
                        if rule.duck_role == DuckRole::Trigger
                            && s.peak >= config.ducking.trigger_threshold
                        {
                            trigger_active = true;
                            break;
                        }
                    }
                }
            }
        }

        let target_duck = if bypass {
            self.duck_hold_ms = 0.0;
            1.0
        } else if trigger_active {
            self.duck_hold_ms = config.ducking.hold_ms;
            (1.0 - config.ducking.reduction).clamp(0.05, 1.0)
        } else if self.duck_hold_ms > 0.0 {
            self.duck_hold_ms = (self.duck_hold_ms - dt).max(0.0);
            (1.0 - config.ducking.reduction).clamp(0.05, 1.0)
        } else {
            1.0
        };

        if target_duck < self.duck_factor {
            let alpha = 1.0 - (-dt / 25.0).exp();
            self.duck_factor += (target_duck - self.duck_factor) * alpha;
        } else {
            let release = config.ducking.release_ms.max(40.0);
            let alpha = 1.0 - (-dt / release).exp();
            self.duck_factor += (target_duck - self.duck_factor) * alpha;
            if (1.0 - self.duck_factor).abs() < 0.003 {
                self.duck_factor = 1.0;
            }
        }

        let ducking_now = self.duck_factor < 0.995;
        let mut decisions = Vec::with_capacity(inputs.len());

        for s in inputs {
            let rule = config.rule_for(&s.key, s.current_volume);
            let rt = self
                .runtimes
                .entry(s.key.clone())
                .or_insert_with(|| SessionRuntime {
                    mode_gain: rule.base_volume,
                    smoothed_peak: s.peak,
                    last_applied: Some(s.current_volume),
                    suppress_sync_ticks: 0,
                });

            if s.peak >= rt.smoothed_peak {
                rt.smoothed_peak = s.peak;
            } else {
                let fall = 1.0 - (-dt / 120.0).exp();
                rt.smoothed_peak += (s.peak - rt.smoothed_peak) * fall;
            }

            let idle_control = bypass
                || (rule.mode == ControlMode::Off
                    && (rule.duck_role != DuckRole::Target || !ducking_now));

            if rt.suppress_sync_ticks > 0 {
                rt.suppress_sync_ticks -= 1;
            } else if idle_control {
                if let Some(last) = rt.last_applied {
                    if (s.current_volume - last).abs() > 0.015 {
                        rule.base_volume = s.current_volume.clamp(0.0, 1.0);
                        rt.mode_gain = rule.base_volume;
                    }
                }
            }

            let base = rule.base_volume.clamp(0.0, 1.0);

            if bypass {
                rt.mode_gain = base;
                let needs_restore = (s.current_volume - base).abs() > 0.001;
                if needs_restore {
                    rt.suppress_sync_ticks = rt.suppress_sync_ticks.max(4);
                }
                rt.last_applied = Some(base);
                decisions.push(SessionDecision {
                    key: s.key.clone(),
                    target_volume: needs_restore.then_some(base),
                    effective_volume: base,
                    display_peak: rt.smoothed_peak,
                    gain_reduction: 0.0,
                    ducking_active: false,
                });
                continue;
            }

            let mode_vol = match rule.mode {
                ControlMode::Off => {
                    rt.mode_gain = base;
                    base
                }
                ControlMode::Limiter => {
                    let ceiling = rule.ceiling.clamp(0.05, 1.0);
                    let cur = s.current_volume.max(0.01);
                    if !s.muted && s.peak > ceiling {
                        let required = (cur * (ceiling / s.peak)).clamp(0.01, base);
                        let attack = 1.0 - (-dt / 12.0).exp();
                        rt.mode_gain = (rt.mode_gain + (required - rt.mode_gain) * attack)
                            .min(required)
                            .clamp(0.01, base);
                    } else {
                        let release = rule.release_ms.max(40.0);
                        let alpha = 1.0 - (-dt / release).exp();
                        rt.mode_gain += (base - rt.mode_gain) * alpha;
                        if (base - rt.mode_gain).abs() < 0.003 {
                            rt.mode_gain = base;
                        }
                    }
                    rt.mode_gain.clamp(0.0, base)
                }
                ControlMode::Leveler => {
                    let gate = rule.noise_gate.clamp(0.005, 0.25);
                    let max_g = rule.max_gain.clamp(0.1, 1.0);
                    let target = rule.target.clamp(0.05, 1.0);
                    rt.mode_gain = rt.mode_gain.clamp(0.05, max_g);
                    if !s.muted && s.peak >= gate {
                        let cur = s.current_volume.max(0.02);
                        let desired = (cur * (target / s.peak)).clamp(0.05, max_g);
                        let time_ms = if desired < rt.mode_gain {
                            80.0
                        } else {
                            rule.release_ms.max(180.0)
                        };
                        let alpha = 1.0 - (-dt / time_ms).exp();
                        rt.mode_gain += (desired - rt.mode_gain) * alpha;
                    }
                    rt.mode_gain.clamp(0.05, max_g)
                }
            };

            let duck_vol = if rule.duck_role == DuckRole::Target && ducking_now {
                base * self.duck_factor
            } else {
                1.0
            };

            let final_vol = if rule.duck_role == DuckRole::Target && ducking_now {
                mode_vol.min(duck_vol).clamp(0.0, 1.0)
            } else {
                mode_vol.clamp(0.0, 1.0)
            };

            let needs_write = (final_vol - s.current_volume).abs() > 0.001;
            if needs_write {
                rt.suppress_sync_ticks = rt.suppress_sync_ticks.max(4);
            }
            rt.last_applied = Some(final_vol);

            decisions.push(SessionDecision {
                key: s.key.clone(),
                target_volume: needs_write.then_some(final_vol),
                effective_volume: final_vol,
                display_peak: rt.smoothed_peak,
                gain_reduction: (base - final_vol).max(0.0),
                ducking_active: rule.duck_role == DuckRole::Target && ducking_now,
            });
        }

        decisions
    }

    pub fn notify_manual_volume(&mut self, key: &str, volume: f32) {
        if let Some(rt) = self.runtimes.get_mut(key) {
            rt.mode_gain = volume;
            rt.last_applied = Some(volume);
            rt.suppress_sync_ticks = 8;
        }
    }

    pub fn suppress_all_sync(&mut self) {
        for rt in self.runtimes.values_mut() {
            rt.suppress_sync_ticks = 8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppRule;

    fn make_input(key: &str, peak: f32, vol: f32) -> SessionInput {
        SessionInput {
            key: key.to_string(),
            peak,
            current_volume: vol,
            muted: false,
        }
    }

    #[test]
    fn limiter_clamps_spike_and_releases_to_base() {
        let mut engine = ControlEngine::new();
        let mut cfg = AppConfig::default();
        cfg.apps.insert(
            "game.exe".into(),
            AppRule {
                mode: ControlMode::Limiter,
                base_volume: 0.8,
                ceiling: 0.4,
                release_ms: 200.0,
                ..Default::default()
            },
        );

        let d = engine.step(&[make_input("game.exe", 0.8, 0.8)], &mut cfg, 16.0);
        let limited = d[0].effective_volume;
        assert!(limited < 0.5, "expected fast attenuation, got {limited}");
        assert!(d[0].gain_reduction > 0.3);

        let mut cur = limited;
        for _ in 0..80 {
            let step = engine.step(&[make_input("game.exe", 0.1, cur)], &mut cfg, 20.0);
            cur = step[0].effective_volume;
        }
        assert!((cur - 0.8).abs() < 0.005, "expected release to base 0.8, got {cur}");
    }

    #[test]
    fn leveler_respects_noise_gate_and_max_gain() {
        let mut engine = ControlEngine::new();
        let mut cfg = AppConfig::default();
        cfg.apps.insert(
            "video.exe".into(),
            AppRule {
                mode: ControlMode::Leveler,
                base_volume: 0.5,
                target: 0.6,
                noise_gate: 0.05,
                max_gain: 0.75,
                release_ms: 200.0,
                ..Default::default()
            },
        );

        let quiet = engine.step(&[make_input("video.exe", 0.01, 0.5)], &mut cfg, 20.0);
        assert!((quiet[0].effective_volume - 0.5).abs() < 0.001);

        let mut cur = 0.5;
        for _ in 0..60 {
            let step = engine.step(&[make_input("video.exe", 0.2, cur)], &mut cfg, 20.0);
            cur = step[0].effective_volume;
        }
        assert!(cur > 0.55 && cur <= 0.7501, "got {cur}");
    }

    #[test]
    fn ducking_attenuates_target_and_recovers() {
        let mut engine = ControlEngine::new();
        let mut cfg = AppConfig::default();
        cfg.ducking.trigger_threshold = 0.05;
        cfg.ducking.reduction = 0.5;
        cfg.ducking.hold_ms = 100.0;
        cfg.ducking.release_ms = 150.0;

        cfg.apps.insert(
            "discord.exe".into(),
            AppRule {
                base_volume: 1.0,
                duck_role: DuckRole::Trigger,
                ..Default::default()
            },
        );
        cfg.apps.insert(
            "chrome.exe".into(),
            AppRule {
                base_volume: 0.8,
                duck_role: DuckRole::Target,
                ..Default::default()
            },
        );

        let mut chrome_vol = 0.8;
        for _ in 0..15 {
            let d = engine.step(
                &[
                    make_input("discord.exe", 0.3, 1.0),
                    make_input("chrome.exe", 0.2, chrome_vol),
                ],
                &mut cfg,
                16.0,
            );
            chrome_vol = d[1].effective_volume;
        }
        assert!(chrome_vol < 0.45, "expected ducked ~0.4, got {chrome_vol}");

        for _ in 0..60 {
            let d = engine.step(
                &[
                    make_input("discord.exe", 0.0, 1.0),
                    make_input("chrome.exe", 0.2, chrome_vol),
                ],
                &mut cfg,
                20.0,
            );
            chrome_vol = d[1].effective_volume;
        }
        assert!((chrome_vol - 0.8).abs() < 0.005, "expected recovery to 0.8, got {chrome_vol}");
    }

    #[test]
    fn bypass_and_off_restore_base_even_with_stale_windows_read() {
        let mut engine = ControlEngine::new();
        let mut cfg = AppConfig::default();
        cfg.apps.insert(
            "app.exe".into(),
            AppRule {
                mode: ControlMode::Limiter,
                base_volume: 0.9,
                ceiling: 0.3,
                ..Default::default()
            },
        );

        let d1 = engine.step(&[make_input("app.exe", 0.9, 0.9)], &mut cfg, 16.0);
        let reduced = d1[0].effective_volume;
        assert!(reduced < 0.5);

        cfg.bypass = true;
        engine.suppress_all_sync();
        let d2 = engine.step(&[make_input("app.exe", 0.9, reduced)], &mut cfg, 16.0);
        assert_eq!(d2[0].target_volume, Some(0.9));
        assert_eq!(cfg.apps["app.exe"].base_volume, 0.9);

        cfg.bypass = false;
        cfg.apps.get_mut("app.exe").unwrap().mode = ControlMode::Off;
        engine.notify_manual_volume("app.exe", 0.9);
        let d3 = engine.step(&[make_input("app.exe", 0.1, reduced)], &mut cfg, 16.0);
        assert_eq!(d3[0].target_volume, Some(0.9));
        assert_eq!(cfg.apps["app.exe"].base_volume, 0.9);
    }
}
