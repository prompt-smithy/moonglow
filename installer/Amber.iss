; Inno Setup script for Amber.
; Compiled by build.ps1 for both the self-contained and framework-dependent flavors.
; The following /D defines are supplied on the ISCC command line:
;   SourceDir           - folder containing the published application to package
;   OutputDir           - folder the generated installer is written to
;   AppVersion          - product version string
;   FrameworkDependent  - defined (value 1) only for the framework-dependent build

#ifndef AppVersion
  #define AppVersion "1.0.0"
#endif

#define MyAppName "Amber"
#define MyAppPublisher "Amber"
#define MyAppExeName "Amber.exe"

[Setup]
; Stable AppId so upgrades replace the previous install instead of stacking.
AppId={{6F3B8B7A-2C4E-4E2A-9E7D-9B1D0C5A7E10}
AppName={#MyAppName}
AppVersion={#AppVersion}
AppVerName={#MyAppName} {#AppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
UninstallDisplayIcon={app}\{#MyAppExeName}
UninstallDisplayName={#MyAppName}
OutputDir={#OutputDir}
; Every flavor produces an installer named Amber.exe.
OutputBaseFilename=Amber
SetupIconFile={#SourcePath}\..\Assets\Amber.ico
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=admin
ArchitecturesInstallIn64BitMode=x64compatible
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Flags: recursesubdirs createallsubdirs ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#MyAppName}}"; Flags: nowait postinstall skipifsilent

#ifdef FrameworkDependent
[Code]
function InitializeSetup(): Boolean;
begin
  Result := True;
  MsgBox('This installer does not bundle the .NET runtime.' + #13#10 +
         'Amber requires the .NET 10 Desktop Runtime (x64).' + #13#10 +
         'If the app fails to start after installation, install the runtime from:' + #13#10 +
         'https://dotnet.microsoft.com/download/dotnet/10.0',
         mbInformation, MB_OK);
end;
#endif
