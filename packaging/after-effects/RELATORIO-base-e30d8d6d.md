> Relatório histórico da base e30d8d6d, antes da publicação no GitHub. As frases sobre commits não enviados e ausência de execução Windows descrevem aquela validação original. Consulte também build.json e os logs do workflow para os resultados de uma nova distribuição nativa.

# EffectCraft — edição experimental de compatibilidade

A entrega é parcial. Há importação funcional de AEP binário por conversor externo e gerenciamento passivo de arquivos AEX. O critério inicial completo ainda não foi atingido: importar FFX e executar/renderizar um plugin AEX continuam pendentes. AEPX também não está implementado.

## O que foi implementado

- Crate `effectcraft-ae-bridge`, separado do engine e da interface, sem `unsafe` e sem vincular decodificadores AEP ou runtimes AEX de terceiros ao editor.
- Importação AEP pelo menu **File → Import → After Effects Project**, pelo comando `file.importAfterEffects` e pelo fluxo de importação/arrastar arquivos.
- Conversão externa sobre uma cópia temporária do AEP; validação antes de alterar o documento; importação em uma pasta com novos IDs, undo/redo e persistência em `.ecproj`.
- Relatório de conversão preservado no comentário da pasta importada, incluindo o caminho original e reparos feitos pela validação. Expressões são preservadas desativadas.
- Limites de tamanho e tempo, captura limitada de erros e recuperação quando o conversor falha. O arquivo original não é enviado ao conversor como caminho gravável.
- Plugin Manager em egui: adicionar/remover pastas, procurar instalações convencionais do Windows, reexaminar, salvar disponibilidade e visualizar diagnósticos. A descoberta lê somente cabeçalhos PE; não executa DLLs nem registra arquivos não testados como efeitos renderizáveis.
- Comandos para CLI/MCP e interface, traduções dos novos menus nos dez idiomas existentes, testes automatizados, script de preparação do conversor, workflow Windows e documentação de arquitetura/matriz de compatibilidade.
- Remoção das imagens de marca restritas da ArtCraft, exigida para versões modificadas. O ícone aberto do EffectCraft foi preservado.
- Ajuste estrutural mínimo no leitor STL existente para atender ao Clippy do workspace, sem mudar seu comportamento; as suítes existentes verificam esse leitor.

## Evidência de funcionamento

Um AEP sintético de 892 bytes, gerado pelo teste público do adaptador externo, foi importado pelo engine: composição 1920×1080 a 30 fps, uma camada, um keyframe em 0,5 segundo e expressão desativada. O teste confirma reabertura do projeto convertido e igualdade dos bytes do AEP antes/depois. A asserção final também verifica o valor de opacidade 80 do keyframe.

Um teste separado do contrato da ponte importa um sólido, verifica pixels vermelhos no renderizador, undo/redo e persistência do relatório e dos parâmetros. Isso testa a integração com o renderizador; não é uma comparação visual com o After Effects.

O script `packaging/after-effects/bootstrap_converter.py` foi executado em um diretório novo: baixou a revisão fixada, compilou o conversor e executou os 40 testes públicos originais do adaptador sem falhas. Outros 24 testes ficaram ignorados por dependerem de arquivos privados/locais. A fixture sintética é explicitamente identificada; nenhum projeto Adobe proprietário é redistribuído.

## O que ainda falta

- Decodificação AEPX e FFX. O menu de presets e arquivos FFX arrastados apresentam erro explícito antes de editar camadas; não há um formato JSON disfarçado de FFX.
- Carregamento/inicialização AEX, descoberta da identidade dos efeitos, parâmetros nativos, processamento 8/16/32-bit, múltiplas instâncias, cache e integração GPU. O gerenciador atual é um inventário passivo.
- Testes com Sapphire, BCC, Twixtor, RSMB, Universe, Twitch e Deep Glow. Nenhum desses plugins comerciais foi executado ou declarado compatível.
- Fidelidade visual contra referências autorizadas do After Effects, corpus amplo, testes de desempenho/memória e políticas rígidas de contenção do processo externo.
- Relink automático das referências de mídia do AEP. Os placeholders ficam identificados; é possível selecionar a mídia e usar o comando existente **Replace Footage → File**.

O `aep-reader` foi mantido externo porque seu próprio `PROVENANCE.md` não afirma origem clean-room independente. Revisão usada: `16cfdb6983621370352a8dd26dcd01845378bb83`, licença MIT OR Apache-2.0. O `aexlo` foi avaliado na revisão `28e9f9a4a371d0ecae0d2b183e167755b3a7cac3`; não foi incorporado. Seu runtime precisa de uma integração externa específica para Windows, com testes reais de ABI e contenção. Não houve tentativa de contornar licenciamento ou DRM.

## Como reproduzir

O código inclui `docs/after-effects-compatibility.md`, com instruções Windows x64, arquitetura, limites e matriz detalhada. O conversor é configurado pelo caminho absoluto em `EFFECTCRAFT_AEP_CONVERTER` antes de abrir o aplicativo.

O processo externo não é um sandbox do sistema operacional: conserva as permissões da conta. Há limites de entrada/saída e tempo, mas não quotas rígidas de memória/CPU nem contenção de processos descendentes. Conversão e varredura de diretórios ainda são síncronas.

A validação global local usa Rust 1.95.0 no ambiente Linux headless padrão; os testes existentes pulam comparações GPU quando não há adaptador. Para reduzir o custo de linkar as 75 suítes de interface no ambiente disponível, o CI completo foi executado com `CARGO_PROFILE_RELEASE_LTO=off` e `CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16`; avisos e testes permanecem habilitados. O build Windows do editor e da CLI usa os mesmos overrides de perfil release em todas as suas dependências e executáveis (`LTO=off`, `CODEGEN_UNITS=16`) para limitar o custo de linkedição. O conversor foi compilado separadamente no perfil release da própria workspace upstream. O desempenho em tempo de execução não foi medido. Resultados finais e identificação dos artefatos constam no registro de validação que acompanha o pacote.

## Executáveis Windows x64

Foram gerados `effectcraft.exe` (editor), `effectcraft-cli.exe` e `aep-effectcraft.exe` (conversor separado). Os três foram identificados como PE32+ x86-64; os imports examinados correspondem a DLLs do próprio Windows. Os símbolos desnecessários foram removidos das cópias de distribuição. O build usa o alvo `x86_64-pc-windows-gnu`, Rust 1.95.0 e GCC 14-posix.

Não houve execução nativa em Windows 11 neste ambiente Linux. A captura `plugin-manager.png` é da interface egui real em Linux, usando renderização por software Mesa/llvmpipe, e não representa uma validação de inicialização no Windows.

Para experimentar o pacote Windows: descompacte o ZIP e abra `Launch.cmd`. Esse arquivo configura o caminho do conversor incluído antes de iniciar o editor. Use **File → Import → After Effects Project**. Consulte o relatório após cada importação; não há suporte completo ao ecossistema After Effects.

O pacote inclui avisos e licenças dos componentes resolvidos, do conversor e dos runtimes de compilação. As instalações comerciais de plugins não fazem parte da entrega.

## Limitação encontrada na validação GPU

Uma execução separada com o driver Vulkan por software Mesa/llvmpipe teve 210 testes GPU aprovados e dois com falha: `tests_fx_pixel2::stylize` (HexTile) e `tests_fx_stylize::posterize_thresholds_strobe_vignette` (Threshold RGB). Esses efeitos já existiam e não foram alterados nesta edição. O registro completo acompanha o código em `delivery/evidence/ci-vulkan-software-failures.log`. A execução headless do CI não resolve nem esconde essa divergência; ela não constitui validação GPU. A captura do gerenciador, renderizada com esse driver, apenas verifica a interface.

## Registro final de validação e histórico

- Branch local: `feat/after-effects-compatibility`.
- Base: `0311bdc34d0dc10c2d18ff7875b4f52d483e5e27`.
- Commit da edição: `e30d8d6dc23af8d95c76072a6e693cbd42b398a9`.
- `cargo xtask ci`: concluído com código de saída 0. Formatação, Clippy com avisos tratados como erro, testes, regras de dependência, atribuição dos 31 assets e checagem WebAssembly de 33 crates passaram. O Clippy é nativo; a etapa WebAssembly emitiu um aviso preexistente sobre o campo não utilizado `State.disabled` em `pointer_wrap.rs:12`.
- O runner nativo registrou 3.429 testes aprovados, zero falhas e 27 ignorados. Esse total inclui helpers GPU que retornam sem comparação quando não há adaptador; ele não equivale a 3.429 validações gráficas. O log de confirmação está em `gpu-default-headless.log`.
- A ponte tem quatro testes de fronteira; os três testes normais do engine e o teste de interface do gerenciador passaram. O teste AEP real, ignorado na suíte padrão por exigir conversor/fixture explícitos, foi executado separadamente e passou. A auditoria de exposição dos comandos por MCP e as duas verificações de catálogo de tradução também passaram.
- O conversor externo passou em 40 testes públicos; 24 testes privados/locais permaneceram ignorados. Os três executáveis Windows foram compilados; não houve execução nativa Windows.
- A verificação separada Mesa Vulkan permanece com duas falhas, documentadas acima. Não foi substituída por uma alegação de compatibilidade.

O ZIP de código inclui o snapshot completo do editor, documentação e testes, além de `delivery/history/COMMITS.txt` e um bundle Git dos commits novos, que requer a revisão-base. O bundle não inclui o histórico anterior de imagens de marca restrita. O snapshot pode ser compilado diretamente sem esse histórico. Nenhum commit foi enviado ao remoto.

O ZIP Windows inclui editor, CLI, conversor separado, `Launch.cmd`, relatório, matriz/arquitetura e avisos de licença de 431 versões de pacotes resolvidos, além dos avisos de assets e toolchain. Os símbolos desnecessários foram removidos das cópias de distribuição. `SHA256SUMS.txt` permite conferir a integridade dos arquivos entregues. A integridade dos ZIPs é verificada pelo empacotador.
