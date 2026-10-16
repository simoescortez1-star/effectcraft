# EffectCraft — compatibilidade avançada experimental

Alvo: Windows 11 x64. Resultado parcial. O commit `e30d8d6d` permanece na ancestralidade;
a distribuição anterior é preservada. `build.json` identifica o commit exato desta build.

## Alterações funcionais

- FFX binário `RIFX/FaFX`: leitura de árvores salvas de propriedades no conversor externo,
  parâmetros, valores, keyframes e expressões desabilitadas. Transformações reconhecidas
  são aplicadas às camadas selecionadas a partir do tempo atual. O mapeador Gaussian
  existente é reutilizado para registros que satisfazem seu contrato restrito.
- Aplicação transacional: validação antes da edição, IDs próprios para novos efeitos,
  desfazer/refazer e persistência em `.ecproj`. Efeitos não disponíveis conservam a árvore
  decodificada e todos os bytes originais do preset no comentário da camada. A interface
  informa quando nada visual foi aplicado; não cria um shake de substituição.
- AEP: referências originais de mídia e um nome derivado do arquivo deixam de ser
  descartados. Imagens reconhecidas conservam a indicação de vídeo. O editor verifica
  caminhos existentes e o relink recursivo entende separadores Windows em outras plataformas.
  Correspondências duplicadas continuam exigindo escolha manual.
- Interpolação: código salvo 2 com handles temporais válidos passa ao modo Bezier
  experimental, conservando velocidades e influências. Antes os handles eram armazenados,
  mas os segmentos permaneciam lineares. Códigos desconhecidos ainda têm aviso/fallback.
- Runtime AEX real em processo separado, baseado em uma revisão fixa de `aexlo`:
  carregamento explícito, inicialização, consulta de sliders float, envio/retorno de
  pixels RGBA float, CPU Smart Pre-Render/Smart Render e integração como efeito comum
  na timeline, preview e renderizador CPU. O editor não carrega a DLL em seu processo.
- Plugin original redistribuível `EffectCraft Gain.aex`, com controle Gain. Serve para
  testar o caminho nativo; não implementa nenhum efeito comercial.
- Erros/saídas do processo auxiliar preservam o frame de entrada. Cada render usa um
  processo novo. Há prazo de 10 segundos e limites de protocolo/arquivos. Frames de
  efeitos AEX experimentais não entram no cache persistente, incluindo dependências de
  outras camadas, para evitar armazenar um fallback como render válido.
- Plugin Manager mostra estágios observados e oferece **Load and test**. A varredura
  continua sem executar plugins. Relatórios AEP/FFX têm resumo e detalhes expansíveis;
  os avisos completos continuam preservados.

## Anexos analisados

Os arquivos privados não são incluídos no repositório nem na Release.

| Entrada | Evidência medida |
| --- | --- |
| `test.aep`, 128.578 bytes | Duas composições 1440×1440 a 60 fps, duas camadas, 14 propriedades mapeadas, dois keyframes de escala e 54 avisos. Uma imagem referenciada não foi fornecida. O segundo keyframe fica em 59/60 s; a escala X chega a 156%. |
| `s shake.ffx`, 47.472 bytes | Um efeito `S_Shake`, 84 filhos de parâmetros, dois keyframes no parâmetro `S_Shake-0050`, ticks originais 0 e 5600. Todos os bytes, inclusive XMP ao final, são preservados. |
| TXT de diagnóstico | 3.784 linhas: 2.516 ocorrências de propriedade não mapeada, 214 estados Unsupported, 11 Partial, 70 definições parciais de efeito, duas ocorrências de valor incompleto e 174 avisos de duração de marcador. Material Options aparece em 1.806 linhas; Time Remapping em 107; Position_0 e Position_1 em 129 cada. As categorias se sobrepõem. |

O TXT se refere a outro projeto maior: seus offsets chegam além de 2 MB e ele relata
139 mídias ausentes. Esses números não foram atribuídos ao `test.aep` de 128 KB.

SHA-256 do AEP: `cd136a5a51c997fce14b919373784042a801e08f324f4897bbba1b928f7d6558`.
SHA-256 do FFX: `f2dce9d5602c32c034433f87e3255b530f924e4a2af22c884d70bfe9d9346b01`.

Não foram fornecidos frames de referência do AE nem a imagem usada no AEP. Renderizar
o projeto com essa mídia ausente verifica estrutura e estabilidade, não fidelidade visual.
Os screenshots fornecidos são diagnósticos da versão anterior, não referências de pixels.

## Validação e reprodução

- A suíte normal inclui regressão da aplicação transacional do carrier FFX e do relink
  de referências Windows. O conversor externo mantém seus testes de tempo, valores,
  estrutura e entradas inválidas; a expectativa de fallback linear foi atualizada
  explicitamente para o novo aviso Bezier.
- `generate_fixtures.py` gera containers FFX binários originais: opacidade animada com
  Bezier e um efeito não disponível. O teste completo verifica pixels transparentes,
  intermediários e opacos, tempo inicial, persistência, undo/redo e dados indisponíveis.
- O teste privado opt-in verifica as medidas do AEP/FFX acima, preservação exata dos
  bytes do preset, ausência de substituição de S_Shake e manutenção dos arquivos originais.
- O workflow Windows compila editor/CLI com Rust 1.95 e MSVC/CRT estático, conversor
  separado e worker/plugin com Rust 1.99. Antes de empacotar, exige os testes reais
  `ffx_external_generated_end_to_end` e
  `aex_native_worker_renders_and_recovers_after_child_exit`.
  Este último verifica pixels com Gain 0,5 em tempos distintos, saída deliberada do
  processo filho, recuperação, undo, persistência e desativação do plugin.
- Os logs desses testes e `WINDOWS-VALIDATION.json` ficam em `validation/` no ZIP.
  O empacotador rejeita evidência de outro commit ou teste que não executou.
  `build.json`, `CLI-SMOKE.txt` e `SHA256SUMS.txt` identificam os binários entregues.
- A publicação é condicionada ao sucesso das etapas do workflow. Uma build bem-sucedida
  não equivale a validar a interface no Windows 11 nem a todos os plugins.

## Limites atuais

- **O preset S_Shake anexado ainda não produz o efeito Sapphire nesta edição.** Seus
  dados são preservados e o resultado informa `applied:false`. Não houve teste de
  renderização com Sapphire, BCC, Twixtor, RSMB, Universe, Deep Glow ou Twitch.
- A associação automática entre match names FFX/AEP e parâmetros de plugins comerciais
  ainda não está implementada/validada. O runtime inicial é um caminho separado,
  aplicado como efeito do registro dinâmico após carregamento manual.
- O clock FFX de 8.000 ticks/s é uma hipótese experimental explícita. Os ticks originais
  são preservados. A correspondência temporal com AE exige arquivos/referências adicionais.
- Interpolação Bezier, unidades de velocidade, 3D, Material Options, texto e efeitos
  avançados ainda exigem comparações de referência; avisos não significam correção visual.
  Expressões importadas permanecem desabilitadas. AEPX não é suportado.
- AEX: somente Windows x64, CPU Smart Render, controles Float Slider, geometria fixa e
  frames até 4.194.304 pixels (máximo de 8192 por dimensão). Não suporta outros tipos de
  parâmetro, GPU, múltiplas entradas, audio, expansão de limites, UI nativa ou todas as suites.
  O tempo nativo é quantizado a 1 ms, sem contexto completo do FPS de origem.
  Um plugin pode exigir callbacks não implementados e falhar. Não há bypass de licenças.
- O worker é isolamento de processo, **não sandbox do sistema operacional**. Reabertura
  não executa automaticamente plugins: carregue manualmente o mesmo binário/local para
  registrar a identidade do efeito. Mudanças de caminho/versão podem exigir recriar o efeito.
- O host externo usa unsafe internamente; o editor não liga esse ABI. A revisão do leitor
  AEP declara limites de proveniência clean-room. O decoder continua externo e não foi
  copiado para crates do editor. Arquivos próprios da edição e patches são publicados.
- O teste da GUI no Windows 11 depende do usuário. O usuário informou que a versão anterior
  abriu no Windows 11; isso não substitui validação desta nova build.
- A base tinha duas divergências em testes GPU Mesa/llvmpipe (HexTile e ThresholdRGB),
  sem alteração de shaders nesta etapa. O relatório histórico acompanha o ZIP. CI headless
  pode pular comparações GPU quando não há adaptador; não se afirma validação GPU completa.

## Instalação e teste

1. Extraia o ZIP inteiro em uma pasta gravável. Execute `Launch.cmd`.
2. Importe o AEP pelo menu File → Import. Abra o relatório e faça relink da imagem ausente
   escolhendo uma pasta raiz com os arquivos reais. Ambiguidades não são resolvidas automaticamente.
3. Selecione uma camada e use Animation → Apply After Effects Preset para o FFX.
   O preset Sapphire ficará preservado até existir suporte validado ao plugin.
4. Para testar o runtime, abra Plugin Manager, adicione a pasta `test-plugins` do ZIP,
   faça Rescan e **Load and test** em EffectCraft Gain. Aplique-o em Effects & Presets →
   Native AEX (experimental). Ajuste Gain. O valor 4 é reservado no fixture para provocar
   a saída do worker e exercitar recuperação; use 0–3,9 para renderização normal.

Pacote portátil, sem MSI nem privilégios de administrador. Binários experimentais sem
assinatura de código. Fontes, licenças e avisos acompanham a distribuição.
