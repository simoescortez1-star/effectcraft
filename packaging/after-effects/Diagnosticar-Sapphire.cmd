@echo off
setlocal
set "EFFECTCRAFT_AEP_CONVERTER=%~dp0aep-effectcraft.exe"
set "EFFECTCRAFT_AEX_WORKER=%~dp0effectcraft-aex-worker.exe"
echo CortezEffects: diagnostico local experimental. Os logs ficam nesta pasta.
echo Executa o Gain incluido e somente os AEX com PiPL exato S_Shake.
echo O FFX, se fornecido, e analisado mas nao aplicado ao plugin.
if "%~1"=="" goto without_preset
"%~dp0effectcraft-aex-diagnostic.exe" --installed --match-name S_Shake --execute-matching --self-test --preset "%~1" --out "%~dp0diagnostico-sapphire.json"
goto done
:without_preset
"%~dp0effectcraft-aex-diagnostic.exe" --installed --match-name S_Shake --execute-matching --self-test --out "%~dp0diagnostico-sapphire.json"
:done
if errorlevel 1 echo Falha no diagnostico. Consulte a mensagem acima.
echo Leia diagnostico-sapphire.json e SAPPHIRE-S_SHAKE.md.
pause
endlocal
