#ifndef MyAppVersion
  #define MyAppVersion "0.1.0"
#endif

[Setup]
AppId={{8E9D4C21-5F4A-4B9E-9A3C-7D1F6B8E2C01}
AppName=Volevel
AppVersion={#MyAppVersion}
AppPublisher=Noah-knd
AppPublisherURL=https://github.com/Noah-knd/volevel
AppSupportURL=https://github.com/Noah-knd/volevel/issues
DefaultDirName={localappdata}\Volevel
DefaultGroupName=Volevel
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=Output
OutputBaseFilename=Volevel-Setup-v{#MyAppVersion}
SetupIconFile=..\assets\icon.ico
UninstallDisplayIcon={app}\volevel.exe
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
AppMutex=Local\VolevelSingleInstance
CloseApplications=yes

[Languages]
Name: "japanese"; MessagesFile: "compiler:Languages\Japanese.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\target\release\volevel.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Volevel"; Filename: "{app}\volevel.exe"
Name: "{autodesktop}\Volevel"; Filename: "{app}\volevel.exe"; Tasks: desktopicon

[UninstallDelete]
Type: filesandordirs; Name: "{localappdata}\volevel"

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueName: "Volevel"; Flags: uninsdeletevalue

[Run]
Filename: "{app}\volevel.exe"; Description: "{cm:LaunchProgram,Volevel}"; Flags: nowait postinstall skipifsilent
