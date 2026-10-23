; Inno Setup - install and register video associations (per-extension type names)
#define MyAppName "暴风影音"
#define MyAppExeName "rust-videos.exe"
#define MyAppPublisher "setsuodu"
#define MyAppURL "https://github.com/setsuodu/rust-videos"

#ifndef MyAppVersion
#define MyAppVersion "0.1.1"
#endif

[Setup]
AppId={{B9E4D3F2-0C5E-5G7B-9F3D-2E6C8B0D1F4A}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
DefaultDirName={localappdata}\rust-videos
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
OutputDir=..\dist
OutputBaseFilename=rust-videos-setup-{#MyAppVersion}
SetupIconFile=..\icon.ico
UninstallDisplayIcon={app}\rust-videos.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
ChangesAssociations=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional options:"; Flags: unchecked
Name: "assoc"; Description: "Set as default app for videos"; GroupDescription: "Additional options:"; Flags: checkedonce

[Files]
Source: "..\target\x86_64-pc-windows-msvc\release\rust-videos.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\rust-videos.exe"
Name: "{group}\Uninstall {#MyAppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\rust-videos.exe"; Tasks: desktopicon

[Registry]
; 每个后缀独立 ProgId，类型列显示「MP4 文件」等，不再显示「暴风影音」
; mp4
Root: HKCU; Subkey: "Software\Classes\rust-videos.mp4"; ValueType: string; ValueName: ""; ValueData: "MP4 文件"; Flags: uninsdeletekey; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\rust-videos.mp4\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\rust-videos.exe,0"; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\rust-videos.mp4\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\rust-videos.exe"" ""%1"""; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\.mp4"; ValueType: string; ValueName: ""; ValueData: "rust-videos.mp4"; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\.mp4\OpenWithProgids"; ValueType: string; ValueName: "rust-videos.mp4"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
; m4v
Root: HKCU; Subkey: "Software\Classes\rust-videos.m4v"; ValueType: string; ValueName: ""; ValueData: "M4V 文件"; Flags: uninsdeletekey; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\rust-videos.m4v\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\rust-videos.exe,0"; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\rust-videos.m4v\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\rust-videos.exe"" ""%1"""; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\.m4v"; ValueType: string; ValueName: ""; ValueData: "rust-videos.m4v"; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\.m4v\OpenWithProgids"; ValueType: string; ValueName: "rust-videos.m4v"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
; webm
Root: HKCU; Subkey: "Software\Classes\rust-videos.webm"; ValueType: string; ValueName: ""; ValueData: "WEBM 文件"; Flags: uninsdeletekey; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\rust-videos.webm\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\rust-videos.exe,0"; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\rust-videos.webm\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\rust-videos.exe"" ""%1"""; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\.webm"; ValueType: string; ValueName: ""; ValueData: "rust-videos.webm"; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\.webm\OpenWithProgids"; ValueType: string; ValueName: "rust-videos.webm"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
; mov
Root: HKCU; Subkey: "Software\Classes\rust-videos.mov"; ValueType: string; ValueName: ""; ValueData: "MOV 文件"; Flags: uninsdeletekey; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\rust-videos.mov\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\rust-videos.exe,0"; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\rust-videos.mov\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\rust-videos.exe"" ""%1"""; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\.mov"; ValueType: string; ValueName: ""; ValueData: "rust-videos.mov"; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\.mov\OpenWithProgids"; ValueType: string; ValueName: "rust-videos.mov"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
; mkv
Root: HKCU; Subkey: "Software\Classes\rust-videos.mkv"; ValueType: string; ValueName: ""; ValueData: "MKV 文件"; Flags: uninsdeletekey; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\rust-videos.mkv\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\rust-videos.exe,0"; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\rust-videos.mkv\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\rust-videos.exe"" ""%1"""; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\.mkv"; ValueType: string; ValueName: ""; ValueData: "rust-videos.mkv"; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\.mkv\OpenWithProgids"; ValueType: string; ValueName: "rust-videos.mkv"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
; ogv
Root: HKCU; Subkey: "Software\Classes\rust-videos.ogv"; ValueType: string; ValueName: ""; ValueData: "OGV 文件"; Flags: uninsdeletekey; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\rust-videos.ogv\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\rust-videos.exe,0"; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\rust-videos.ogv\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\rust-videos.exe"" ""%1"""; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\.ogv"; ValueType: string; ValueName: ""; ValueData: "rust-videos.ogv"; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\.ogv\OpenWithProgids"; ValueType: string; ValueName: "rust-videos.ogv"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
; avi
Root: HKCU; Subkey: "Software\Classes\rust-videos.avi"; ValueType: string; ValueName: ""; ValueData: "AVI 文件"; Flags: uninsdeletekey; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\rust-videos.avi\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\rust-videos.exe,0"; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\rust-videos.avi\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\rust-videos.exe"" ""%1"""; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\.avi"; ValueType: string; ValueName: ""; ValueData: "rust-videos.avi"; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\.avi\OpenWithProgids"; ValueType: string; ValueName: "rust-videos.avi"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
; wmv
Root: HKCU; Subkey: "Software\Classes\rust-videos.wmv"; ValueType: string; ValueName: ""; ValueData: "WMV 文件"; Flags: uninsdeletekey; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\rust-videos.wmv\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\rust-videos.exe,0"; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\rust-videos.wmv\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\rust-videos.exe"" ""%1"""; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\.wmv"; ValueType: string; ValueName: ""; ValueData: "rust-videos.wmv"; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\.wmv\OpenWithProgids"; ValueType: string; ValueName: "rust-videos.wmv"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc

; 清理旧统一 ProgId（卸载时删）
Root: HKCU; Subkey: "Software\Classes\rust-videos.file"; Flags: uninsdeletekey; Tasks: assoc

[Run]
Filename: "{app}\rust-videos.exe"; Description: "Launch {#MyAppName}"; Flags: nowait postinstall skipifsilent
