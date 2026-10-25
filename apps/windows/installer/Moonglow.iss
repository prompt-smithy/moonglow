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
SetupIconFile={#SourcePath}\..\..\..\resources\Moonglow.ico
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
function GetCurrentProcessId: DWORD;
  external 'GetCurrentProcessId@kernel32.dll stdcall';

// Moonglow.exe --shutdown signals a running same-generation copy (matched by install
// path) and exits at once; the copy then needs a moment (gamma restore, DDC release).
// Wait for the app family to vanish so files are unlocked and the post-install launch
// does not trip the single-instance guard. Never force-kill: killing skips the
// gamma-ramp restore and leaves the screen tinted.
function AnyFamilyProcessRunning(IncludeMoonglow: Boolean): Boolean;
var
  Wbem, Processes: Variant;
  Query: String;
begin
  Result := False;
  try
    Query := 'SELECT ProcessId FROM Win32_Process WHERE (';
    if IncludeMoonglow then
      Query := Query + 'Name LIKE ''Moonglow%.exe'' OR ';
    Query := Query + 'Name LIKE ''Amber%.exe'') AND ProcessId <> ' +
      IntToStr(GetCurrentProcessId());
    Wbem := CreateOleObject('WbemScripting.SWbemLocator');
    Processes := Wbem.ConnectServer('.', 'root\CIMV2').ExecQuery(Query);
    Result := Processes.Count > 0;
  except
  end;
end;

procedure WaitForFamilyExit(IncludeMoonglow: Boolean);
var
  I: Integer;
begin
  for I := 0 to 80 do
  begin
    if not AnyFamilyProcessRunning(IncludeMoonglow) then
      Exit;
    Sleep(250);
  end;
end;

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

  // The pre-merge Amber / Night Mode Controller generation drives the same display
  // gamma ramps, so Moonglow refuses to start beside it. Catch that here -- with a
  // chance to close it -- instead of failing the first launch after install.
  while AnyFamilyProcessRunning(False) do
  begin
    if MsgBox('Setup detected the older Amber (Night Mode Controller) app still running.' + #13#10 +
              'Moonglow cannot run while it is active.' + #13#10 + #13#10 +
              'Right-click the Amber tray icon and choose Exit, then click Retry.' + #13#10 +
              'Click Cancel to abort Setup.',
              mbConfirmation, MB_RETRYCANCEL) = IDCANCEL then
    begin
      Result := False;
      Exit;
    end;
  end;
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

procedure CurStepChanged(CurStep: TSetupStep);
var
  OldExe: String;
  Code: Integer;
begin
  // Gracefully stop the previous copy before replacing files: the tray app has no
  // window for the standard close flow, and a lingering copy trips the
  // single-instance guard on the post-install launch.
  if CurStep = ssInstall then
  begin
    OldExe := ExpandConstant('{app}\Moonglow.exe');
    if FileExists(OldExe) then
    begin
      Exec(OldExe, '--shutdown', '', SW_HIDE, ewWaitUntilTerminated, Code);
      WaitForFamilyExit(True);
    end;
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

