# Installs the `mutastic` command on Windows, for Rust, Maven and Gradle projects alike, from
# PowerShell 5.1 or 7:
#   irm https://mutastic.io/install.ps1 | iex
# or, from a clone of mutastic's repository:
#   powershell -ExecutionPolicy Bypass -File scripts\install.ps1
#
# Downloads the native mutastic for this machine (x64 or Arm64) from a GitHub release, checks it
# against the release's SHA256SUMS, keeps it in %USERPROFILE%\.mutastic\native-<version>, copies
# mutastic.exe and cargo-mutastic.exe into %USERPROFILE%\.mutastic\bin, and adds that folder to your
# user PATH. Run it again to update. In a Cargo project mutastic runs itself; the first time it runs
# in a Maven or Gradle project, it fetches its Java edition into %USERPROFILE%\.mutastic too, which
# needs JDK 25. Removing that folder, and its bin folder from the user PATH, uninstalls it.
#
# Settings, each optional:
#   MUTASTIC_NATIVE_VERSION  the release of the native command to install; the latest unless set
#   MUTASTIC_HOME            where releases go, %USERPROFILE%\.mutastic unless set
#   MUTASTIC_BIN_DIR         where the commands are copied, the bin folder in MUTASTIC_HOME unless set
#   MUTASTIC_NATIVE_RELEASES where releases are downloaded from, for a mirror: a URL, or a folder
#                            laid out as one (download\v<version>\...), with MUTASTIC_NATIVE_VERSION
#
# `iex` runs this in your own session, so nothing here calls `exit`, which would close it: a problem
# stops it with an error instead. Everything is in one script block, run on the last line, so a
# download cut short runs nothing and its functions don't stay in your session.

& {
    function Install-Mutastic {
        $ErrorActionPreference = 'Stop'
        # Invoke-WebRequest's progress bar slows a download in PowerShell 5.1 many times over.
        $ProgressPreference = 'SilentlyContinue'
        # PowerShell 5.1 may not offer TLS 1.2, which GitHub needs, unless asked.
        [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

        $releases = $env:MUTASTIC_NATIVE_RELEASES
        if (-not $releases) { $releases = 'https://github.com/mutastic/mutastic-rs/releases' }
        $releases = $releases.TrimEnd('/', '\')
        $target = Get-MutasticTarget
        $version = $env:MUTASTIC_NATIVE_VERSION
        if (-not $version) { $version = Get-MutasticLatest $releases }
        $mutasticHome = $env:MUTASTIC_HOME
        if (-not $mutasticHome) { $mutasticHome = Join-Path $env:USERPROFILE '.mutastic' }
        $bin = $env:MUTASTIC_BIN_DIR
        if (-not $bin) { $bin = Join-Path $mutasticHome 'bin' }
        $name = "mutastic-$version-$target"

        $work = Join-Path ([IO.Path]::GetTempPath()) "mutastic-install-$([Guid]::NewGuid())"
        New-Item -ItemType Directory -Path $work | Out-Null
        try {
            Write-Mutastic "downloading $version for $target"
            $zip = Join-Path $work "$name.zip"
            try {
                Get-MutasticFile $releases "download/v$version/$name.zip" $zip
            } catch {
                throw "mutastic: couldn't download $version for ${target}: is it a release? ($releases)"
            }
            Confirm-MutasticChecksum $releases $version $zip "$name.zip" (Join-Path $work 'SHA256SUMS')

            Expand-Archive -LiteralPath $zip -DestinationPath $work
            $unpacked = Join-Path $work $name
            if (-not (Test-Path -LiteralPath (Join-Path $unpacked 'mutastic.exe') -PathType Leaf)) {
                throw 'mutastic: the download had no mutastic.exe in it'
            }
            $kept = Join-Path $mutasticHome "native-$version"
            New-Item -ItemType Directory -Force -Path $mutasticHome, $bin | Out-Null
            if (Test-Path -LiteralPath $kept) { Remove-Item -LiteralPath $kept -Recurse -Force }
            Move-Item -LiteralPath $unpacked -Destination $kept
            foreach ($command in 'mutastic.exe', 'cargo-mutastic.exe') {
                try {
                    Copy-Item -LiteralPath (Join-Path $kept $command) -Destination (Join-Path $bin $command) -Force
                } catch {
                    throw "mutastic: couldn't copy $command into ${bin}: is it running?"
                }
            }
        } finally {
            Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
        }

        Write-Mutastic "installed ${version}: $(Join-Path $bin 'mutastic.exe')"
        Write-Mutastic 'in a Maven or Gradle project, it fetches its Java edition the first time, which runs on JDK 25'
        if (Add-MutasticToPath $bin) {
            Write-Mutastic "added $bin to your user PATH: open a new terminal, then, in a Cargo, Maven or Gradle project: mutastic run"
        } else {
            Write-Mutastic 'next, in a Cargo, Maven or Gradle project: mutastic run'
        }
    }

    function Write-Mutastic([string] $message) {
        Write-Host "mutastic: $message"
    }

    # The release target for this machine, as Rust names it: the machine's own processor, from the
    # system environment, as an x64 PowerShell runs on Arm64 too and says AMD64 in its own.
    function Get-MutasticTarget {
        $arch = [Environment]::GetEnvironmentVariable('PROCESSOR_ARCHITECTURE', 'Machine')
        if (-not $arch) { $arch = $env:PROCESSOR_ARCHITECTURE }
        switch ($arch) {
            'AMD64' { return 'x86_64-pc-windows-msvc' }
            'ARM64' { return 'aarch64-pc-windows-msvc' }
            default { throw "mutastic: there's no mutastic release for Windows on $arch yet: build it from source, as the install page says" }
        }
    }

    # The newest release's version, from where GitHub's "latest release" page redirects to.
    function Get-MutasticLatest([string] $releases) {
        if ($releases -notmatch '^https?://') {
            throw "mutastic: set MUTASTIC_NATIVE_VERSION to the release to install from $releases"
        }
        try {
            $request = [Net.WebRequest]::Create("$releases/latest")
            $request.Method = 'HEAD'
            $response = $request.GetResponse()
            $url = $response.ResponseUri.AbsoluteUri
            $response.Close()
        } catch {
            throw "mutastic: couldn't reach $releases to find the latest release"
        }
        if ($url -match '/v(?<version>\d+\.\d+\.\d+)$') { return $Matches['version'] }
        throw "mutastic: couldn't tell the latest release from $url"
    }

    # A release's file, from a URL, or from a folder laid out as releases are.
    function Get-MutasticFile([string] $releases, [string] $path, [string] $to) {
        if ($releases -match '^https?://') {
            Invoke-WebRequest -UseBasicParsing -Uri "$releases/$path" -OutFile $to
        } else {
            Copy-Item -LiteralPath (Join-Path $releases ($path -replace '/', '\')) -Destination $to
        }
    }

    # Checks the download against the release's SHA256SUMS: every release has one.
    function Confirm-MutasticChecksum([string] $releases, [string] $version, [string] $file, [string] $listed, [string] $sums) {
        try {
            Get-MutasticFile $releases "download/v$version/SHA256SUMS" $sums
        } catch {
            throw "mutastic: couldn't download the release's SHA256SUMS: not installed"
        }
        $expected = $null
        foreach ($line in Get-Content -LiteralPath $sums) {
            $sum, $named = $line -split '\s+', 2
            if ($named -and $named.TrimStart('*') -eq $listed) { $expected = $sum.ToLowerInvariant() }
        }
        if (-not $expected) { throw "mutastic: SHA256SUMS doesn't list ${listed}: not installed" }
        $actual = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($actual -ne $expected) { throw "mutastic: the download doesn't match its checksum: not installed" }
    }

    # Adds `bin` to the user PATH in the registry, where every new terminal reads it, and to this
    # session's: false when it's there already. The value is read and written as it's kept, so the
    # entries that name another variable (%USERPROFILE%\...) still do.
    function Add-MutasticToPath([string] $bin) {
        $environment = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
        try {
            $userPath = [string] $environment.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
            $entries = @($userPath -split ';' | Where-Object { $_ })
            $already = $entries | Where-Object { [Environment]::ExpandEnvironmentVariables($_).TrimEnd('\') -eq $bin.TrimEnd('\') }
            if (-not $already) {
                $environment.SetValue('Path', (($entries + $bin) -join ';'), [Microsoft.Win32.RegistryValueKind]::ExpandString)
            }
        } finally {
            $environment.Close()
        }
        if (($env:Path -split ';') -notcontains $bin) { $env:Path = "$env:Path;$bin" }
        if ($already) { return $false }
        # Setting a user variable through .NET tells Windows the environment changed, so a terminal
        # opened from now on has the new PATH without signing out. This one is set and taken back.
        [Environment]::SetEnvironmentVariable('MutasticPathChanged', '1', 'User')
        [Environment]::SetEnvironmentVariable('MutasticPathChanged', $null, 'User')
        return $true
    }

    Install-Mutastic
}
