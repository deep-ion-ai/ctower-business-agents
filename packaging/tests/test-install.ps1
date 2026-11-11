<#
Testes do packaging/install/install.ps1 sem release real (rodam no Windows).

Monta uma release falsa em disco, aponta o script para ela (CBA_BASE_URL=file:///...)
e roda em modo de teste (CBA_DRY_RUN=1), que baixa e confere o SHA-256 sem instalar nada.

Uso: powershell -NoProfile -File packaging/tests/test-install.ps1 [-Shell pwsh]
#>
param([string]$Shell = 'powershell')

$ErrorActionPreference = 'Continue'
$script = (Resolve-Path (Join-Path $PSScriptRoot '..\install\install.ps1')).Path
$asset = 'cba-windows-x64-setup.exe'
$falhas = 0

function Initialize-Cenario {
    $raiz = Join-Path ([IO.Path]::GetTempPath()) ('cba-test-' + [Guid]::NewGuid())
    New-Item -ItemType Directory -Path (Join-Path $raiz 'rel\download\v0.1.0'), (Join-Path $raiz 'app') | Out-Null
    return $raiz
}

function Publish-Release([string]$Raiz, [string]$Hash) {
    $pasta = Join-Path $Raiz 'rel\download\v0.1.0'
    $exe = Join-Path $pasta $asset
    [IO.File]::WriteAllBytes($exe, [Text.Encoding]::ASCII.GetBytes('instalador falso'))
    if (-not $Hash) { $Hash = (Get-FileHash -Algorithm SHA256 $exe).Hash.ToLower() }
    Set-Content -Path (Join-Path $pasta 'SHA256SUMS') -Value @("$('0' * 64)  cba-macos-universal.dmg", "$Hash  $asset") -Encoding Ascii
}

# Roda o instalador num processo novo com as variaveis CBA_* e devolve codigo e saida.
function Invoke-Instalador([string]$Raiz, [string[]]$Extra, [switch]$ScriptBlock) {
    $env:CBA_BASE_URL = 'file:///' + (Join-Path $Raiz 'rel').Replace('\', '/').TrimStart('/')
    $env:CBA_INSTALL_DIR = Join-Path $Raiz 'app'
    $env:CBA_VERSION = '0.1.0'
    try {
        if ($ScriptBlock) {
            $cmd = "& ([scriptblock]::Create((Get-Content -Raw '$script'))) $($Extra -join ' ')"
            $saida = & $Shell -NoProfile -NonInteractive -Command $cmd 2>&1 | Out-String
        } else {
            $saida = & $Shell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $script @Extra 2>&1 | Out-String
        }
        return @{ Codigo = $LASTEXITCODE; Saida = $saida }
    } finally {
        Remove-Item env:CBA_BASE_URL, env:CBA_INSTALL_DIR, env:CBA_VERSION -ErrorAction SilentlyContinue
    }
}

function Assert-Teste([string]$Nome, [bool]$Ok, $Resultado) {
    if ($Ok) { Write-Host "ok   $Nome" } else {
        Write-Host "FALHA $Nome (codigo $($Resultado.Codigo))`n$($Resultado.Saida)"
        $script:falhas++
    }
}

# Hash certo passa.
$r0 = Initialize-Cenario; Publish-Release $r0
$r = Invoke-Instalador $r0 @('-DryRun')
Assert-Teste 'hash certo passa' ($r.Codigo -eq 0 -and $r.Saida -match "SHA-256 conferido: $asset" -and $r.Saida -match 'nada foi instalado') $r

# Hash divergente aborta com codigo diferente de zero e mensagem clara.
$r1 = Initialize-Cenario; Publish-Release $r1 ('ab' * 32)
$r = Invoke-Instalador $r1 @('-DryRun')
Assert-Teste 'hash divergente aborta' ($r.Codigo -ne 0 -and $r.Saida -match 'SHA-256 nao confere' -and $r.Saida -match 'Instalacao abortada') $r

# Mesma variavel de ambiente de iex: CBA_DRY_RUN, invocado como scriptblock com parametro.
$env:CBA_DRY_RUN = '1'
$r = Invoke-Instalador $r0 @('-Yes') -ScriptBlock
Remove-Item env:CBA_DRY_RUN
Assert-Teste 'scriptblock com -Yes e CBA_DRY_RUN' ($r.Codigo -eq 0 -and $r.Saida -match 'SHA-256 conferido') $r

# Mesma versao ja instalada: nao baixa (a release falsa nem existe).
$r2 = Initialize-Cenario
$exe = Join-Path $r2 'app\cba.exe'
Copy-Item (Get-Command $Shell).Source $exe
$v = (Get-Item $exe).VersionInfo.ProductVersion
if ($v -match '^(\d+)\.(\d+)\.(\d+)') { $versao = "$($Matches[1]).$($Matches[2]).$($Matches[3])" }
$env:CBA_DRY_RUN = '1'
$r = Invoke-Instalador $r2 @('-Version', $versao)
Remove-Item env:CBA_DRY_RUN
Assert-Teste 'mesma versao nao reinstala' ($r.Codigo -eq 0 -and $r.Saida -match "versao $versao ja instalada" -and $r.Saida -notmatch 'Baixando') $r

# Log em arquivo.
Assert-Teste 'log em arquivo' (Select-String -Path (Join-Path $r0 'app\install.log') -Pattern 'SHA-256 conferido' -Quiet) @{ Codigo = 0; Saida = '' }

# Sem terminal e sem -Yes: nao faz nada.
$r = Invoke-Instalador $r0 @()
Assert-Teste 'sem -Yes e sem terminal aborta' ($r.Codigo -ne 0 -and $r.Saida -match 'sem terminal') $r

Remove-Item -Recurse -Force $r0, $r1, $r2 -ErrorAction SilentlyContinue
if ($falhas) { Write-Host "$falhas teste(s) falharam."; exit 1 }
Write-Host 'Todos os testes passaram.'
