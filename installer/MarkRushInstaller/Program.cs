using System.Diagnostics;
using System.Reflection;
using System.Text;
using Microsoft.Win32;

const string AppName = "MarkRush";
const string Publisher = "flashosophy";

var version = ThisAssemblyVersion();
var installDir = Path.Combine(
    Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
    "Programs",
    AppName);
var fontLicenseDir = Path.Combine(installDir, "font-licenses");
var startMenuDir = Environment.GetFolderPath(Environment.SpecialFolder.StartMenu);
var shortcutPath = Path.Combine(startMenuDir, "Programs", "MarkRush.lnk");
var uninstallScriptPath = Path.Combine(installDir, "Uninstall-MarkRush.ps1");

Directory.CreateDirectory(installDir);
Directory.CreateDirectory(fontLicenseDir);

foreach (var resourceName in Assembly.GetExecutingAssembly().GetManifestResourceNames())
{
    if (!resourceName.StartsWith("Payload\\", StringComparison.OrdinalIgnoreCase))
    {
        continue;
    }

    var fileName = resourceName["Payload\\".Length..];
    var destination = DestinationPath(fileName, installDir, fontLicenseDir);
    Directory.CreateDirectory(Path.GetDirectoryName(destination)!);

    await using var source = Assembly.GetExecutingAssembly().GetManifestResourceStream(resourceName)
        ?? throw new InvalidOperationException($"Missing installer resource: {resourceName}");
    await using var target = File.Create(destination);
    await source.CopyToAsync(target);
}

WriteUninstaller(uninstallScriptPath, installDir, shortcutPath);
CreateStartMenuShortcut(shortcutPath, Path.Combine(installDir, "markrush.exe"), installDir);
RegisterUninstallEntry(version, installDir, uninstallScriptPath);

Console.WriteLine($"MarkRush {version} installed to {installDir}");

static string DestinationPath(string fileName, string installDir, string fontLicenseDir)
{
    return fileName switch
    {
        "font-licenses-README.md" => Path.Combine(fontLicenseDir, "README.md"),
        "OFL-Lora.txt" or "OFL-NotoSans.txt" or "OFL-NotoSerif.txt" or "OFL-Poppins.txt" or "OFL-RobotoMono.txt"
            => Path.Combine(fontLicenseDir, fileName),
        _ => Path.Combine(installDir, fileName),
    };
}

static string ThisAssemblyVersion()
{
    return Assembly.GetExecutingAssembly()
        .GetCustomAttribute<AssemblyInformationalVersionAttribute>()
        ?.InformationalVersion
        ?? "dev";
}

static void WriteUninstaller(string uninstallScriptPath, string installDir, string shortcutPath)
{
    var script = $$"""
        $ErrorActionPreference = 'SilentlyContinue'

        $InstallDir = '{{EscapePowerShellSingleQuoted(installDir)}}'
        $ShortcutPath = '{{EscapePowerShellSingleQuoted(shortcutPath)}}'
        $UninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\MarkRush'

        Remove-Item -LiteralPath $ShortcutPath -Force
        Remove-Item -LiteralPath $UninstallKey -Recurse -Force

        $CleanupCommand = "Start-Sleep -Seconds 1; Remove-Item -LiteralPath '$InstallDir' -Recurse -Force"
        Start-Process powershell.exe -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command', $CleanupCommand) -WindowStyle Hidden
        """;

    File.WriteAllText(uninstallScriptPath, script, new UTF8Encoding(encoderShouldEmitUTF8Identifier: false));
}

static void CreateStartMenuShortcut(string shortcutPath, string targetPath, string workingDirectory)
{
    Directory.CreateDirectory(Path.GetDirectoryName(shortcutPath)!);

    var script = $$"""
        $Shell = New-Object -ComObject WScript.Shell
        $Shortcut = $Shell.CreateShortcut('{{EscapePowerShellSingleQuoted(shortcutPath)}}')
        $Shortcut.TargetPath = '{{EscapePowerShellSingleQuoted(targetPath)}}'
        $Shortcut.WorkingDirectory = '{{EscapePowerShellSingleQuoted(workingDirectory)}}'
        $Shortcut.IconLocation = '{{EscapePowerShellSingleQuoted(targetPath)}}'
        $Shortcut.Description = 'MarkRush Markdown viewer and editor'
        $Shortcut.Save()
        """;

    RunPowerShell(script);
}

static void RegisterUninstallEntry(string version, string installDir, string uninstallScriptPath)
{
    using var key = Registry.CurrentUser.CreateSubKey(@"Software\Microsoft\Windows\CurrentVersion\Uninstall\MarkRush");
    key.SetValue("DisplayName", "MarkRush", RegistryValueKind.String);
    key.SetValue("DisplayVersion", version, RegistryValueKind.String);
    key.SetValue("Publisher", Publisher, RegistryValueKind.String);
    key.SetValue("InstallLocation", installDir, RegistryValueKind.String);
    key.SetValue("DisplayIcon", Path.Combine(installDir, "markrush.exe"), RegistryValueKind.String);
    key.SetValue(
        "UninstallString",
        $"powershell.exe -NoProfile -ExecutionPolicy Bypass -File \"{uninstallScriptPath}\"",
        RegistryValueKind.String);
    key.SetValue("NoModify", 1, RegistryValueKind.DWord);
    key.SetValue("NoRepair", 1, RegistryValueKind.DWord);
}

static void RunPowerShell(string script)
{
    using var process = Process.Start(new ProcessStartInfo
    {
        FileName = "powershell.exe",
        ArgumentList =
        {
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        },
        UseShellExecute = false,
        CreateNoWindow = true,
    }) ?? throw new InvalidOperationException("Could not start powershell.exe");

    process.WaitForExit();
    if (process.ExitCode != 0)
    {
        throw new InvalidOperationException($"PowerShell exited with code {process.ExitCode}");
    }
}

static string EscapePowerShellSingleQuoted(string value) => value.Replace("'", "''", StringComparison.Ordinal);
