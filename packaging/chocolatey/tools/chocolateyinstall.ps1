$ErrorActionPreference = 'Stop'
$toolsDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$packageArgs = @{
    packageName   = 'rozvp'
    fileType      = 'exe'
    url           = 'https://github.com.mlm-games.rozvp/releases/latest'
    softwareName  = 'rozvp'
    checksum      = ''
    checksumType  = 'sha256'
}
Install-ChocolateyPackage @packageArgs
