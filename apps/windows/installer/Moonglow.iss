; Inno Setup script for Moonglow.
; Compiled by build.ps1, which wraps the portable .NET Framework 4.8 build.
; The following /D defines are supplied on the ISCC command line:
;   SourceDir           - folder containing the published application to package
;   OutputDir           - folder the generated installer is written to
;   AppVersion          - product version string

#ifndef AppVersion
  #define AppVersion "1.0.0"
#endif

#define MyAppName "Moonglow"
#define MyAppPublisher "Moonglow"
#define MyAppExeName "Moonglow.exe"

[Setup]
; Stable AppId so upgrades replace the previous install instead of stacking.
AppId={{6F3B8B7A-2C4E-4E2A-9E7D-9B1D0C5A7E10}
AppName={#MyAppName}
AppVersion={#AppVersion}
AppVerName={#MyAppName} {#AppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={autopf}\Moonglow
DefaultGroupName={#MyAppName}
UninstallDisplayIcon={app}\{#MyAppExeName}
UninstallDisplayName={#MyAppName}
OutputDir={#OutputDir}
; Every flavor produces an installer named Moonglow.exe.
OutputBaseFilename=Moonglow
SetupIconFile={#SourcePath}\..\Assets\Moonglow.ico
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

[UninstallDelete]
Type: filesandordirs; Name: "{app}\*"

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#MyAppName}}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{app}\{#MyAppExeName}"; Parameters: "--shutdown"; Flags: runhidden waituntilterminated skipifdoesntexist; RunOnceId: "CloseMoonglow"

[Code]
// .NET Framework 4.8 reported as release 528040 and later. Moonglow targets net48, so warn
// (but do not block) when it is missing - Windows 10 1903 and later already include it.
// https://learn.microsoft.com/dotnet/framework/migration-guide/how-to-determine-which-versions-are-installed
function IsDotNet48Installed(): Boolean;
var
  Release: Cardinal;
begin
  if RegQueryDWordValue(HKLM, 'SOFTWARE\Microsoft\NET Framework Setup\NDP\v4\Full', 'Release', Release) then
    Result := Release >= 528040
  else
    Result := False;
end;

function InitializeSetup(): Boolean;
begin
  Result := True;
  if not IsDotNet48Installed() then
  begin
    if MsgBox('Moonglow requires the .NET Framework 4.8 runtime, which was not detected on this system.' + #13#10 +
              'Windows 10 1903 and later include it. On older versions of Windows, install it from:' + #13#10 +
              'https://dotnet.microsoft.com/download/dotnet-framework/net48' + #13#10 + #13#10 +
              'Continue with the installation anyway?',
              mbConfirmation, MB_YESNO) = IDNO then
      Result := False;
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
  begin
    RegDeleteValue(HKCU, 'SOFTWARE\Microsoft\Windows\CurrentVersion\Run', 'Moonglow');
    // Remove entries left by earlier product names.
    RegDeleteValue(HKCU, 'SOFTWARE\Microsoft\Windows\CurrentVersion\Run', 'Amber');
    RegDeleteValue(HKCU, 'SOFTWARE\Microsoft\Windows\CurrentVersion\Run', 'Night Mode Controller');
  end;
end;

