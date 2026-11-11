<#
Arquivo so com ASCII de proposito: o PowerShell 5.1 le .ps1 sem BOM como ANSI.

Instalador do Ctower Business Agents para Windows (PowerShell 5.1 ou 7).

Recomendado: baixe, leia e so entao rode.
  irm https://github.com/deep-ion-ai/ctower-business-agents/releases/latest/download/install.ps1 -OutFile install.ps1
  notepad install.ps1
  powershell -ExecutionPolicy Bypass -File install.ps1
Atalho:        irm https://github.com/deep-ion-ai/ctower-business-agents/releases/latest/download/install.ps1 | iex
Sem perguntas: $env:CBA_YES=1; irm https://github.com/deep-ion-ai/ctower-business-agents/releases/latest/download/install.ps1 | iex
          ou:  & ([scriptblock]::Create((irm https://github.com/deep-ion-ai/ctower-business-agents/releases/latest/download/install.ps1))) -Yes

O que faz, sempre mostrando o plano antes de executar:
  - baixa o instalador do app da release, confere o SHA-256 publicado na mesma release
    e instala por usuario em %LOCALAPPDATA%\Ctower Business Agents, com atalho no menu Iniciar;
  - instala o git (winget, por usuario) e o WebView2 (Windows 10, instalador oficial da
    Microsoft, por usuario) se faltarem;
  - mostra como instalar o Claude Code e o Claude Desktop (fluxos oficiais da Anthropic).
Nao pede administrador (exceto -FromSource, que avisa antes), nao pede nem guarda credenciais
e nao reinstala o que ja existe.

Opcoes (ou variaveis de ambiente):
  -Yes            CBA_YES=1          nao pergunta nada
  -Version X.Y.Z  CBA_VERSION        versao especifica (padrao: a mais recente)
  -FromSource     CBA_FROM_SOURCE=1  compila do codigo-fonte (exige as Visual Studio Build Tools)
  -DryRun         CBA_DRY_RUN=1      mostra o plano, baixa e confere o app, e nao muda nada
  -BaseUrl URL    CBA_BASE_URL       origem das releases (espelhos e testes)
So para testes: CBA_INSTALL_DIR troca a pasta de instalacao.
Log: %LOCALAPPDATA%\Ctower Business Agents\install.log
#>
param(
    [switch]$Yes,
    [string]$Version,
    [switch]$FromSource,
    [switch]$DryRun,
    [string]$BaseUrl
)

function Install-Cba {
    param([bool]$Yes, [string]$Version, [bool]$FromSource, [bool]$DryRun, [string]$BaseUrl)

    # Variaveis de preferencia valem so dentro desta funcao (importante com `irm | iex`).
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue'
    $AssumeYes = $Yes
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

    $Repo = 'deep-ion-ai/ctower-business-agents'
    $AppName = 'Ctower Business Agents'
    $Asset = 'cba-windows-x64-setup.exe'
    $NodeMajor = 24
    if (-not $BaseUrl) { $BaseUrl = "https://github.com/$Repo/releases" }
    $Version = "$Version".TrimStart('v')
    $InstallDir = if ($env:CBA_INSTALL_DIR) { $env:CBA_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA $AppName }
    $Exe = Join-Path $InstallDir 'cba.exe'
    $ToolsDir = Join-Path $env:LOCALAPPDATA 'cba'
    $Log = Join-Path $InstallDir 'install.log'
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    $Tmp = Join-Path ([IO.Path]::GetTempPath()) ("cba-install-" + [Guid]::NewGuid())
    New-Item -ItemType Directory -Force -Path $Tmp | Out-Null

    function Add-LogLine([string]$Message) {
        Add-Content -Path $Log -Value ("{0} {1}" -f (Get-Date -Format s), $Message) -Encoding UTF8
    }
    function Write-Info([string]$Message) { Write-Host $Message; Add-LogLine $Message }
    function Exit-Install([string]$Message) { Add-LogLine "ERRO: $Message"; throw "ERRO: $Message" }

    function Confirm-Step([string]$Question) {
        if ($AssumeYes) { return $true }
        $semTerminal = (-not [Environment]::UserInteractive) -or ([Environment]::GetCommandLineArgs() -contains '-NonInteractive')
        if ($semTerminal) { Exit-Install 'sem terminal para confirmar. Rode de novo com -Yes ou $env:CBA_YES=1.' }
        return ((Read-Host "$Question [s/N]") -match '^(s|sim|y|yes)$')
    }

    function Get-File([string]$Url, [string]$Destination) {
        try { (New-Object Net.WebClient).DownloadFile($Url, $Destination) }
        catch { Exit-Install "nao consegui baixar ${Url}: $($_.Exception.Message)" }
    }

    # Test-Hash <arquivo> <nome na lista> <arquivo com "hash  nome" por linha>
    function Test-Hash([string]$File, [string]$Name, [string]$Sums) {
        $esperado = ''
        foreach ($linha in Get-Content -Path $Sums) {
            $partes = "$linha".Trim() -split '\s+', 2
            if ($partes.Count -eq 2 -and $partes[1].TrimStart('*') -eq $Name) { $esperado = $partes[0].ToLower(); break }
        }
        if (-not $esperado) { Exit-Install "o arquivo de hashes nao lista $Name. Instalacao abortada; nada foi instalado." }
        $obtido = (Get-FileHash -Algorithm SHA256 -Path $File).Hash.ToLower()
        if ($esperado -ne $obtido) {
            Exit-Install "SHA-256 nao confere para $Name (esperado $esperado, obtido $obtido). O arquivo pode estar corrompido ou adulterado. Instalacao abortada; nada foi instalado."
        }
        Write-Info "SHA-256 conferido: $Name"
    }

    function Invoke-Step([string]$File, [string[]]$Arguments) {
        Write-Info "+ $File $($Arguments -join ' ')"
        $p = Start-Process -FilePath $File -ArgumentList $Arguments -Wait -PassThru -NoNewWindow
        if ($p.ExitCode -ne 0) { Exit-Install "falhou (codigo $($p.ExitCode)): $File" }
    }

    # Comando de console com a saida na tela e no log.
    function Invoke-Logged([string]$Command, [string[]]$Arguments) {
        Write-Info "+ $Command $($Arguments -join ' ')"
        $ErrorActionPreference = 'Continue'
        & $Command @Arguments 2>&1 | ForEach-Object { $l = "$_"; Write-Host $l; Add-Content -Path $Log -Value $l -Encoding UTF8 }
        if ($LASTEXITCODE -ne 0) { Exit-Install "falhou (codigo $LASTEXITCODE): $Command $($Arguments -join ' ')" }
    }

    function Sync-Path {
        $env:Path = [Environment]::GetEnvironmentVariable('Path', 'User') + ';' + [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + $env:Path
    }

    # A pagina /releases/latest redireciona para /releases/tag/vX.Y.Z.
    function Get-LatestVersion {
        try {
            $req = [Net.HttpWebRequest]::Create("$BaseUrl/latest")
            $req.Method = 'HEAD'
            $req.AllowAutoRedirect = $false
            $resp = $req.GetResponse()
            $loc = $resp.Headers['Location']
            $resp.Close()
            if ($loc -match '/tag/v([^/]+)$') { return $Matches[1] }
        } catch { Add-LogLine "versao mais recente nao resolvida: $($_.Exception.Message)" }
        return ''
    }

    function Get-InstalledVersion {
        if (-not (Test-Path $Exe)) { return '' }
        $v = (Get-Item $Exe).VersionInfo.ProductVersion
        if ("$v" -match '^(\d+)\.(\d+)\.(\d+)') { return "$($Matches[1]).$($Matches[2]).$($Matches[3])" }
        return ''
    }

    function Test-Git { [bool](Get-Command git -ErrorAction SilentlyContinue) }
    function Test-ClaudeCode {
        [bool](Get-Command claude -ErrorAction SilentlyContinue) -or (Test-Path (Join-Path $env:USERPROFILE '.local\bin\claude.exe'))
    }
    function Test-ClaudeDesktop {
        if (Test-Path (Join-Path $env:LOCALAPPDATA 'AnthropicClaude')) { return $true }
        try { return [bool](Get-AppxPackage -Name '*Claude*' -ErrorAction SilentlyContinue) } catch { return $false }
    }
    # Runtime do WebView2 (ja vem no Windows 11; no 10 pode faltar), por maquina ou por usuario.
    function Test-WebView2 {
        $id = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
        foreach ($k in @("HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\$id",
                "HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\$id",
                "HKCU:\Software\Microsoft\EdgeUpdate\Clients\$id")) {
            $item = Get-ItemProperty -Path $k -Name pv -ErrorAction SilentlyContinue
            if ($item -and $item.pv -and $item.pv -ne '0.0.0.0') { return $true }
        }
        return $false
    }
    function Get-VsBuildToolPath {
        $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
        if (-not (Test-Path $vswhere)) { return '' }
        return "$(& $vswhere -products * -latest -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath)"
    }

    # --- Passos ---------------------------------------------------------------

    function Install-Git {
        if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
            Write-Info 'git: o winget nao esta disponivel. Instale o Git for Windows por https://git-scm.com/downloads/win'
            if ($FromSource) { Exit-Install 'a compilacao precisa do git.' }
            return
        }
        Invoke-Step 'winget' @('install', '--id', 'Git.Git', '-e', '--scope', 'user', '--silent', '--accept-package-agreements', '--accept-source-agreements')
        Sync-Path
    }

    # Bootstrapper oficial da Microsoft; sem administrador, instala por usuario.
    function Install-WebView2 {
        $setup = Join-Path $Tmp 'MicrosoftEdgeWebview2Setup.exe'
        Get-File 'https://go.microsoft.com/fwlink/p/?LinkId=2124703' $setup
        $sig = Get-AuthenticodeSignature -FilePath $setup
        if ($sig.Status -ne 'Valid' -or $sig.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation') {
            Exit-Install 'o instalador do WebView2 baixado nao tem assinatura valida da Microsoft. Instalacao abortada.'
        }
        Invoke-Step $setup @('/silent', '/install')
    }

    function Get-App {
        Write-Info "Baixando $Asset de $Dl"
        $setup = Join-Path $Tmp $Asset
        Get-File "$Dl/SHA256SUMS" (Join-Path $Tmp 'SHA256SUMS')
        Get-File "$Dl/$Asset" $setup
        Test-Hash $setup $Asset (Join-Path $Tmp 'SHA256SUMS')
        return $setup
    }

    # O NSIS do Tauri, por usuario, aceita /S e cria o atalho no menu Iniciar.
    function Install-App([string]$Setup) {
        Invoke-Step $Setup @('/S')
        if (-not (Test-Path $Exe)) { Exit-Install "o instalador terminou, mas nao achei $Exe." }
        Write-Info "App instalado em $InstallDir"
    }

    function Confirm-Shortcut {
        $programas = [Environment]::GetFolderPath('Programs')
        $existente = Get-ChildItem -Path $programas -Filter "$AppName.lnk" -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($existente) { return }
        $atalho = (New-Object -ComObject WScript.Shell).CreateShortcut((Join-Path $programas "$AppName.lnk"))
        $atalho.TargetPath = $Exe
        $atalho.WorkingDirectory = $InstallDir
        $atalho.Save()
        Write-Info 'Atalho criado no menu Iniciar.'
    }

    function Install-VsBuildTool {
        Write-Info 'As Visual Studio Build Tools exigem administrador: o Windows vai pedir confirmacao (UAC).'
        Invoke-Step 'winget' @('install', '--id', 'Microsoft.VisualStudio.2022.BuildTools', '-e', '--accept-package-agreements', '--accept-source-agreements',
            '--override', '"--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"')
    }

    function Install-Rust {
        $cargoBin = Join-Path $(if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }) 'bin'
        $env:Path = "$cargoBin;$env:Path"
        if (Get-Command rustup -ErrorAction SilentlyContinue) { return }
        Write-Info 'Instalando o rustup na pasta do usuario (sem alterar o PATH do sistema).'
        $url = 'https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe'
        $init = Join-Path $Tmp 'rustup-init.exe'
        Get-File $url $init
        Get-File "$url.sha256" (Join-Path $Tmp 'rustup-init.sha256')
        $h = ((Get-Content (Join-Path $Tmp 'rustup-init.sha256') -Raw).Trim() -split '\s+')[0]
        Set-Content -Path (Join-Path $Tmp 'rustup-sums') -Value "$h  rustup-init.exe"
        Test-Hash $init 'rustup-init.exe' (Join-Path $Tmp 'rustup-sums')
        Invoke-Step $init @('-y', '--no-modify-path', '--profile', 'minimal', '--default-toolchain', 'none')
    }

    # Node oficial (zip de nodejs.org) em %LOCALAPPDATA%\cba\node, conferido pelo SHASUMS256.
    function Install-Node {
        $nodeDir = Join-Path $ToolsDir 'node'
        $env:Path = "$nodeDir;$env:Path"
        $node = Get-Command node -ErrorAction SilentlyContinue
        if ($node -and [int]((& node -p 'process.versions.node.split(".")[0]')) -ge 20) { return }
        $dist = "https://nodejs.org/dist/latest-v$NodeMajor.x"
        $sums = Join-Path $Tmp 'node-sums'
        Get-File "$dist/SHASUMS256.txt" $sums
        $nome = (Get-Content $sums | ForEach-Object { ($_ -split '\s+')[1] } | Where-Object { $_ -match '^node-v[\d.]+-win-x64\.zip$' } | Select-Object -First 1)
        if (-not $nome) { Exit-Install "nao achei o Node $NodeMajor para win-x64." }
        Write-Info "Instalando $nome em $nodeDir."
        $zip = Join-Path $Tmp $nome
        Get-File "$dist/$nome" $zip
        Test-Hash $zip $nome $sums
        Expand-Archive -Path $zip -DestinationPath $Tmp -Force
        if (Test-Path $nodeDir) { Remove-Item -Recurse -Force $nodeDir }
        New-Item -ItemType Directory -Force -Path $ToolsDir | Out-Null
        Move-Item (Join-Path $Tmp ($nome -replace '\.zip$', '')) $nodeDir
    }

    # Usa o diretorio atual se ja for o repositorio; senao, clona em %LOCALAPPDATA%\cba\src.
    # Supoe o app Tauri em ui\ com o script "tauri" no package.json (#3).
    function Build-FromSource {
        try { $origem = "$(git remote get-url origin 2>$null)" } catch { $origem = '' }
        if ((Test-Path 'ui\package.json') -and $origem -match 'ctower-business-agents') {
            $src = (Get-Location).Path
            Write-Info "Usando o repositorio atual: $src"
        } else {
            $src = Join-Path $ToolsDir 'src'
            if (-not (Test-Path (Join-Path $src '.git'))) { Invoke-Logged 'git' @('clone', '-q', '--depth', '1', "https://github.com/$Repo.git", $src) }
            if ($Version) {
                Invoke-Logged 'git' @('-C', $src, 'fetch', '-q', '--depth', '1', 'origin', 'tag', "v$Version")
                Invoke-Logged 'git' @('-C', $src, 'checkout', '-q', "v$Version")
            }
        }
        Install-Rust
        Install-Node
        Push-Location $src
        try {
            Invoke-Logged 'rustup' @('toolchain', 'install')
            Set-Location 'ui'
            Invoke-Logged 'npm' @('ci')
            Invoke-Logged 'npm' @('run', 'tauri', 'build', '--', '--bundles', 'nsis')
        } finally { Pop-Location }
        $setup = Get-ChildItem -Path (Join-Path $src 'target\release\bundle\nsis'), (Join-Path $src 'ui\src-tauri\target\release\bundle\nsis') -Filter '*-setup.exe' -ErrorAction SilentlyContinue | Select-Object -First 1
        if (-not $setup) { Exit-Install 'a compilacao nao gerou o instalador NSIS.' }
        Install-App $setup.FullName
    }

    # Claude Code e Claude Desktop: so os fluxos oficiais da Anthropic.
    # Enquanto a #43 (confirmacao com a Anthropic) nao for respondida, o script so MOSTRA o
    # comando oficial. Se a resposta permitir, este e o lugar de perguntar (Confirm-Step) e
    # executar o instalador oficial sem modificacao; ate la, nenhuma opcao o executa.
    function Show-Claude {
        if (Test-ClaudeCode) {
            Write-Info "Claude Code: ja instalado. Se ainda nao fez login, rode 'claude' e use /login."
        } else {
            Write-Info 'Claude Code: nao encontrado. Para instalar, rode no PowerShell o instalador oficial da Anthropic:'
            Write-Info '    irm https://claude.ai/install.ps1 | iex'
            Write-Info "  Depois abra um terminal novo, rode 'claude' e faca login com /login. O login e seu: este script nao toca em credenciais."
        }
        if (-not (Test-ClaudeDesktop)) {
            Write-Info 'Claude Desktop (opcional, para quem prefere nao usar terminal): https://claude.ai/download'
        }
    }

    # --- Principal ------------------------------------------------------------

    try {
        Write-Info "== $AppName`: instalacao (Windows) =="
        if (-not $Version) { $Version = Get-LatestVersion }
        $Dl = if ($Version) { "$BaseUrl/download/v$Version" } else { "$BaseUrl/latest/download" }

        $atual = Get-InstalledVersion
        $needApp = -not ($atual -and $atual -eq $Version)
        $needGit = -not (Test-Git)
        $needWebView = -not (Test-WebView2)
        $needBuildTools = $FromSource -and $needApp -and -not (Get-VsBuildToolPath)

        Write-Info ''
        Write-Info 'Plano:'
        if (-not $needApp) {
            Write-Info "  - app: versao $atual ja instalada; nada a fazer"
        } elseif ($FromSource) {
            if ($needBuildTools) { Write-Info '  - instalar as Visual Studio Build Tools (EXIGE ADMINISTRADOR: o Windows vai pedir confirmacao)' }
            Write-Info "  - instalar rustup e Node $NodeMajor na pasta do usuario, se faltarem"
            Write-Info "  - compilar a versao $(if ($Version) { $Version } else { 'mais recente' }) (npm ci && npm run tauri build) e instalar"
        } else {
            $alvo = if ($Version) { $Version } else { 'mais recente (nao consegui descobrir o numero; reinstala)' }
            $tinha = if ($atual) { " (instalada: $atual)" } else { '' }
            Write-Info "  - app: instalar a versao $alvo$tinha em $InstallDir, conferindo o SHA-256"
        }
        if ($needGit) { Write-Info '  - git: instalar com winget, por usuario' } else { Write-Info '  - git: ja instalado' }
        if ($needWebView) { Write-Info '  - WebView2: instalar pelo instalador oficial da Microsoft, por usuario' } else { Write-Info '  - WebView2: ja instalado' }
        if (Test-ClaudeCode) { Write-Info '  - Claude Code: ja instalado' } else { Write-Info '  - Claude Code: mostrar o comando oficial de instalacao (nao executa)' }
        Write-Info "  Log: $Log"
        Write-Info ''

        if ($DryRun) {
            if ($needApp -and -not $FromSource) { Get-App | Out-Null }
            Write-Info 'Modo de teste (-DryRun): nada foi instalado.'
            return
        }
        if (-not (Confirm-Step 'Continuar?')) { Exit-Install 'cancelado; nada foi feito.' }

        if ($needGit) { Install-Git }
        if ($needWebView) { Install-WebView2 }
        if ($needBuildTools) { Install-VsBuildTool }
        if ($needApp -and $FromSource) { Build-FromSource }
        elseif ($needApp) { Install-App (Get-App) }
        if (Test-Path $Exe) { Confirm-Shortcut }
        Write-Info ''
        Show-Claude
        if (Test-Path $Exe) { Start-Process -FilePath $Exe } else { Write-Info "Nao achei $Exe para abrir." }
        Write-Info 'Pronto.'
    } finally {
        Remove-Item -Recurse -Force -Path $Tmp -ErrorAction SilentlyContinue
    }
}

function Get-CbaOption([string]$Value, [string]$EnvName) { if ($Value) { $Value } else { [Environment]::GetEnvironmentVariable($EnvName) } }

Install-Cba -Yes ($Yes -or $env:CBA_YES -eq '1') `
    -Version (Get-CbaOption $Version 'CBA_VERSION') `
    -FromSource ($FromSource -or $env:CBA_FROM_SOURCE -eq '1') `
    -DryRun ($DryRun -or $env:CBA_DRY_RUN -eq '1') `
    -BaseUrl (Get-CbaOption $BaseUrl 'CBA_BASE_URL')
