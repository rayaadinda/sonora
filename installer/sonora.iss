#ifndef AppVersion
  #define AppVersion "1.0.0"
#endif

[Setup]
AppId={{8E4F7D2A-9B3C-4E12-A5F6-71C8D3E94B20}
AppName=Sonora
AppVersion={#AppVersion}
AppVerName=Sonora {#AppVersion}
AppPublisher=Sonora
DefaultDirName={autopf}\Sonora
DefaultGroupName=Sonora
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
ChangesEnvironment=yes
OutputDir=..\dist
OutputBaseFilename=sonora-v{#AppVersion}-windows-x86_64-setup

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "addtopath"; Description: "Add Sonora to PATH (run 'sonora' from any terminal)"; GroupDescription: "Additional options:"
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\target\release\sonora.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Sonora"; Filename: "{app}\sonora.exe"; WorkingDir: "{app}"
Name: "{autodesktop}\Sonora"; Filename: "{app}\sonora.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Run]
Filename: "{app}\sonora.exe"; WorkingDir: "{app}"; Description: "{cm:LaunchProgram,Sonora}"; Flags: nowait postinstall skipifsilent shellexec

[Code]
function EnvRootKey: Integer;
begin
  if IsAdminInstallMode then
    Result := HKEY_LOCAL_MACHINE
  else
    Result := HKEY_CURRENT_USER;
end;

function EnvSubKey: String;
begin
  if IsAdminInstallMode then
    Result := 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment'
  else
    Result := 'Environment';
end;

procedure AddToPath(Dir: String);
var
  Paths: String;
begin
  if not RegQueryStringValue(EnvRootKey, EnvSubKey, 'Path', Paths) then
    Paths := '';

  if Pos(';' + UpperCase(Dir) + ';', ';' + UpperCase(Paths) + ';') > 0 then
    Exit;

  if (Length(Paths) > 0) and (Paths[Length(Paths)] <> ';') then
    Paths := Paths + ';';

  Paths := Paths + Dir;
  RegWriteExpandStringValue(EnvRootKey, EnvSubKey, 'Path', Paths);
end;

procedure RemoveFromPath(Dir: String);
var
  Paths: String;
  Target: String;
  P: Integer;
begin
  if not RegQueryStringValue(EnvRootKey, EnvSubKey, 'Path', Paths) then
    Exit;

  Target := ';' + UpperCase(Dir) + ';';
  P := Pos(Target, ';' + UpperCase(Paths) + ';');
  if P = 0 then
    Exit;

  Paths := ';' + Paths + ';';
  Delete(Paths, P, Length(Dir) + 1);
  if (Length(Paths) > 0) and (Paths[1] = ';') then
    Delete(Paths, 1, 1);
  if (Length(Paths) > 0) and (Paths[Length(Paths)] = ';') then
    Delete(Paths, Length(Paths), 1);

  RegWriteExpandStringValue(EnvRootKey, EnvSubKey, 'Path', Paths);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if (CurStep = ssPostInstall) and WizardIsTaskSelected('addtopath') then
    AddToPath(ExpandConstant('{app}'));
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    RemoveFromPath(ExpandConstant('{app}'));
end;
