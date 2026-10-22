# Installs the airsl command-line tool.
#
#   powershell -ExecutionPolicy Bypass -c "irm https://airsstack.github.io/airsl/install.ps1 | iex"
#
#   # or, inside a PowerShell session, pinned to one release:
#   $env:AIRSL_VERSION = '0.1.4'; irm https://airsstack.github.io/airsl/install.ps1 | iex
#
# This is a pointer, not an installer. It fetches the installer dist generated
# for one release and runs it, so the short URL never goes stale and what it
# installs is exactly what that release published. Every variable that
# installer reads (AIRSL_CLI_INSTALL_DIR, AIRSL_CLI_NO_MODIFY_PATH, ...) passes
# through untouched. dist's installer refuses to run under a Restricted
# execution policy, hence the Bypass above.
#
# Published to GitHub Pages from install/ by .github/workflows/pages.yml: a
# change merged here is live for everyone who runs the command above.

# Run as `irm | iex`, this text executes in the caller's own session. The script
# block keeps every variable and preference it sets out of that session.
& {
    $ErrorActionPreference = 'Stop'

    $repo = 'https://github.com/airsstack/airsl'
    $installer = 'airsl-cli-installer.ps1'

    $version = if ($env:AIRSL_VERSION) { $env:AIRSL_VERSION.TrimStart('v') } else { 'latest' }
    if ($version -eq 'latest') {
        $url = "$repo/releases/latest/download/$installer"
        $what = 'the latest release'
    } elseif ($version -match '^[0-9A-Za-z.+-]+$') {
        $url = "$repo/releases/download/airsl-cli-v$version/$installer"
        $what = "airsl $version"
    } else {
        throw "airsl-install: error: AIRSL_VERSION='$env:AIRSL_VERSION' is not a version (expected something like 0.1.4)"
    }

    # Windows PowerShell 5.1 can still default to TLS 1.0, which GitHub refuses.
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

    Write-Host "airsl-install: fetching the installer for $what"
    try {
        # WebClient, as dist's installer uses: it returns a string on both
        # Windows PowerShell and PowerShell 7, whatever content type is served.
        $script = (New-Object System.Net.WebClient).DownloadString($url)
    } catch {
        if ($version -eq 'latest') {
            throw "airsl-install: error: could not download ${url}: $($_.Exception.Message)"
        }
        throw "airsl-install: error: could not download the installer for $what; is $version a released version? See $repo/releases"
    }

    & ([scriptblock]::Create($script)) @args
} @args
