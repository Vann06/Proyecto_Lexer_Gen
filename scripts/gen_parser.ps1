# Regenera backend/dbms/parser/ a partir de backend/grammar/SQL.g4.
# Requiere `pip install antlr4-tools` (descarga Java y el jar de ANTLR la
# primera vez). El resultado se versiona: ni Docker ni CI necesitan Java.
$ErrorActionPreference = "Stop"
Push-Location (Join-Path $PSScriptRoot "..\backend\grammar")
try {
    antlr4 -v 4.13.2 -Dlanguage=Python3 -visitor -no-listener -Xexact-output-dir -o ../dbms/parser SQL.g4
    if ($LASTEXITCODE -ne 0) { throw "antlr4 falló" }
} finally { Pop-Location }
