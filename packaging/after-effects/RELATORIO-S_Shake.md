# EffectCraft — análise S_Shake e diagnóstico AEX local

Data: 2026-10-16. Alvo: Windows 11 x64. **Sapphire/S_Shake não está declarado compatível.**
O critério completo — preset aplicado e frames produzidos pelo Sapphire original — continua
pendente: o vínculo FFX→parâmetros nativos ainda precisa de implementação validada, e o host
precisa ser testado na instalação licenciada do usuário. Não foi implementada uma imitação.

## Entradas efetivamente analisadas

| Arquivo | Bytes | SHA-256 |
| --- | ---: | --- |
| `s shake.ffx` | 47472 | `f2dce9d5602c32c034433f87e3255b530f924e4a2af22c884d70bfe9d9346b01` |
| `teste s shake.aep` | 181924 | `5bfb46c4dfd43cd04cde585f8a3720f28e4100eac498777f798ec75f094289c0` |
| `Comp 1_27.mp4` | 1499963 | `97c551c4cf992f79bbcae06764b002a66c6140b9f99690d972db43eec2525051` |

O FFX é idêntico por hash ao S_Shake fornecido na etapa anterior. O documento de requisitos
foi tratado como instrução do usuário; AEP, FFX e vídeo foram tratados como dados de teste.
Nenhum desses binários privados, frame de referência ou plugin comercial é distribuído.

## FFX: estrutura e parâmetros decodificados

Contêiner `RIFX/FaFX`, extensão RIFF de 43866 bytes e 3606 bytes adicionais de XMP.
O efeito tem match name **`S_Shake`**, lido do registro, independentemente do nome do arquivo.
A árvore tem 88 nós: 85 propriedades e 3 grupos, incluindo o efeito raiz. Há 84 filhos diretos
sob o efeito; 83 propriedades têm valor estático, uma tem duas chaves e uma contém dados
personalizados não decodificados. Isso não significa 85 controles editáveis do Sapphire:
há botões, registros de grupo, referências e opções de composição do host.

O clock salvo é **23976 ticks/s**, concordante entre `beso` e os registros `tdb4` lidos.
Não é usado o antigo palpite de 8000 ticks/s. Não foi aplicada uma correção arbitrária para
60 fps. Tempos e valores abaixo são os efetivamente retornados pelo leitor externo fixado
em `aep-reader 16cfdb6983621370352a8dd26dcd01845378bb83` e pelo overlay desta edição.
Os números são valores brutos: por exemplo, `Opacity=2` não é convertido em uma porcentagem
inventada; enumerações não recebem nomes de opções ausentes.

### Valores encontrados

| Match name salvo | Nome salvo | Tipo indicado pelo leitor | Valor bruto | Observação |
| --- | --- | --- | --- | --- |
| `S_Shake-0000` | (sem nome salvo) | LayerIndex | 0.0 |  |
| `S_Shake-0239` | Load Preset | OneD | 0.0 | definição parcial |
| `S_Shake-0238` | Save Preset | OneD | 0.0 | definição parcial |
| `S_Shake-0002` | Mask from Layer | LayerIndex | 0.0 |  |
| `S_Shake-0235` | Mask from Path | MaskIndex | 0.0 |  |
| `S_Shake-0001` | Style | OneD | 1.0 |  |
| `S_Shake-0520` | mocha | Custom | não decodificado | dados personalizados incompletos |
| `S_Shake-0241` | Edit Mocha | OneD | 0.0 | definição parcial |
| `S_Shake-0250` | Mocha | NoValue | 0.0 |  |
| `S_Shake-0521` | Blur Mocha | OneD | 0.0 |  |
| `S_Shake-0522` | Mocha Opacity | OneD | 1.0 |  |
| `S_Shake-0523` | Invert Mocha | OneD | 0.0 |  |
| `S_Shake-0524` | Resize Mocha | OneD | 1.0 |  |
| `S_Shake-0525` | Resize Rel X | OneD | 1.0 |  |
| `S_Shake-0526` | Resize Rel Y | OneD | 1.0 |  |
| `S_Shake-0527` | Shift Mocha X | OneD | 0.0 |  |
| `S_Shake-0528` | Shift Mocha Y | OneD | 0.0 |  |
| `S_Shake-0531` | Bypass Mocha | OneD | 0.0 |  |
| `S_Shake-0529` | Show Mocha Only | OneD | 0.0 |  |
| `S_Shake-0530` | Combine Masks | OneD | 1.0 |  |
| `S_Shake-0251` | (sem nome salvo) | OneD | 0.0 | definição parcial |
| `S_Shake-0050` | Amplitude | OneD | animado: 1.4 → 0.0 |  |
| `S_Shake-0051` | Frequency | OneD | 8.0 |  |
| `S_Shake-0052` | Phase | OneD | 0.0 |  |
| `S_Shake-0110` | Stillness | OneD | 0.6999969482421875 |  |
| `S_Shake-0111` | Twitch Frequency | OneD | 2.0 |  |
| `S_Shake-0113` | Drift | OneD | 0.29998779296875 |  |
| `S_Shake-0114` | Center Bias | OneD | 0.0 |  |
| `S_Shake-0053` | Z Dist | OneD | 1.0 |  |
| `S_Shake-0054` | Motion Blur | OneD | 1.0 |  |
| `S_Shake-0055` | Mo Blur Length | OneD | 10.0 |  |
| `S_Shake-0056` | Seed | OneD | 0.26 |  |
| `S_Shake-0057` | Wrap X | OneD | 3.0 |  |
| `S_Shake-0058` | Wrap Y | OneD | 3.0 |  |
| `S_Shake-0252` | X Shake | NoValue | 0.0 |  |
| `S_Shake-0059` | X Rand Amp | OneD | 192.0 |  |
| `S_Shake-0060` | X Rand Freq | OneD | 1.0 |  |
| `S_Shake-0061` | X Wave Amp | OneD | 0.0 |  |
| `S_Shake-0062` | X Wave Freq | OneD | 0.5 |  |
| `S_Shake-0063` | X Phase | OneD | 0.0 |  |
| `S_Shake-0253` | (sem nome salvo) | OneD | 0.0 | definição parcial |
| `S_Shake-0254` | Y Shake | NoValue | 0.0 |  |
| `S_Shake-0064` | Y Rand Amp | OneD | 96.0 |  |
| `S_Shake-0065` | Y Rand Freq | OneD | 1.0 |  |
| `S_Shake-0066` | Y Wave Amp | OneD | 0.0 |  |
| `S_Shake-0067` | Y Wave Freq | OneD | 0.5 |  |
| `S_Shake-0068` | Y Phase | OneD | 0.0 |  |
| `S_Shake-0255` | (sem nome salvo) | OneD | 0.0 | definição parcial |
| `S_Shake-0256` | Z Shake | NoValue | 0.0 |  |
| `S_Shake-0069` | Z Rand Amp | OneD | 0.0 |  |
| `S_Shake-0070` | Z Rand Freq | OneD | 1.0 |  |
| `S_Shake-0071` | Z Wave Amp | OneD | 0.0 |  |
| `S_Shake-0072` | Z Wave Freq | OneD | 0.5 |  |
| `S_Shake-0073` | Z Phase | OneD | 0.0 |  |
| `S_Shake-0257` | (sem nome salvo) | OneD | 0.0 | definição parcial |
| `S_Shake-0258` | Tilt Shake | NoValue | 0.0 |  |
| `S_Shake-0074` | Tilt Rand Amp | OneD | 0.0 |  |
| `S_Shake-0075` | Tilt Rand Freq | OneD | 1.0 |  |
| `S_Shake-0076` | Tilt Wave Amp | OneD | 0.0 |  |
| `S_Shake-0077` | Tilt Wave Freq | OneD | 0.5 |  |
| `S_Shake-0078` | Tilt Phase | OneD | 0.0 |  |
| `S_Shake-0259` | (sem nome salvo) | OneD | 0.0 | definição parcial |
| `S_Shake-0260` | Channels | NoValue | 0.0 |  |
| `S_Shake-0100` | Red Amplitude | OneD | 1.0 |  |
| `S_Shake-0101` | Green Amplitude | OneD | 1.0 |  |
| `S_Shake-0102` | Blue Amplitude | OneD | 1.0 |  |
| `S_Shake-0103` | Red Phase | OneD | 0.0 |  |
| `S_Shake-0104` | Green Phase | OneD | 0.0 |  |
| `S_Shake-0105` | Blue Phase | OneD | 0.0 |  |
| `S_Shake-0106` | RGB Randomness | OneD | 0.0 |  |
| `S_Shake-0107` | RGB Frequency | OneD | 2.0 |  |
| `S_Shake-0261` | (sem nome salvo) | OneD | 0.0 | definição parcial |
| `S_Shake-0079` | Opacity | OneD | 2.0 |  |
| `S_Shake-0540` | Mask Use | OneD | 1.0 |  |
| `S_Shake-0541` | Blur Mask | OneD | 12.0 |  |
| `S_Shake-0542` | Invert Mask | OneD | 0.0 |  |
| `S_Shake-0262` | Crop Input | NoValue | 0.0 |  |
| `S_Shake-0502` | Crop Left | OneD | 0.0 |  |
| `S_Shake-0503` | Crop Right | OneD | 0.0 |  |
| `S_Shake-0504` | Crop Top | OneD | 0.0 |  |
| `S_Shake-0505` | Crop Bottom | OneD | 0.0 |  |
| `S_Shake-0263` | (sem nome salvo) | OneD | 0.0 | definição parcial |
| `S_Shake-0249` | Help | OneD | 0.0 | definição parcial |
| `ADBE Effect Mask Opacity` | ADBE Effect Mask Opacity | OneD | 100.0 |  |
| `ADBE Force CPU GPU` | ADBE Force CPU GPU | OneD | 1.0 |  |

### Animação de Amplitude (`S_Shake-0050`)

| Chave | Ticks | Segundos (ticks / 23976) | Valor | Interpolação de entrada / saída |
| --- | ---: | ---: | ---: | --- |
| 1 | 0 | 0 | 1.4 | 2 / 2 |
| 2 | 5600 | 0.2335669002335669 | 0.0 | 2 / 2 |

| Chave | Velocidade de entrada | Influência de entrada | Velocidade de saída | Influência de saída |
| --- | ---: | ---: | ---: | ---: |
| 1 | 0 | 0.33333333 | -108.78974665502274 | 0.042814285714285714 |
| 2 | -0.0034564804510817163 | 0.7400906029928153 | 0 | 0.16666666667 |

Os valores de influência acima são frações brutas, sem multiplicação por 100. Cada chave
contém um componente de ease, flags brutas `[0,0]` e tangentes espaciais vazias.
O código 2 é tratado como Bezier pelo mapeador experimental existente, mas a unidade das
velocidades e a reprodução dessa curva para um parâmetro nativo Sapphire **não estão
verificadas contra o After Effects**. As duas chaves estão preservadas como dados do preset;
ainda não são chaves de uma instância Sapphire na timeline. “0 chaves aplicadas” no relatório
de importação significa falta de mapeamento para o efeito, não ausência de chaves no FFX.

### Estruturas ainda desconhecidas / preservadas

- `S_Shake-0520` (`mocha`): valor `Custom` incompleto, registro em byte 19848, comprimento 190.
  Nenhum valor foi inventado para substituir esse bloco.
- Onze definições de controles são parciais: `0239`, `0238`, `0241`, `0251`, `0253`, `0255`,
  `0257`, `0259`, `0261`, `0263`, `0249`, todos sob o prefixo `S_Shake-`.
  Alguns têm zeros salvos, mas isso não os torna sliders numéricos.
- Sem instalação Sapphire testada, a correspondência entre sufixos FFX, IDs persistentes
  `PF_ParamDef.uu.id`, índices nativos, tipos, escalas e unidades continua não validada.
  Não se faz associação automática por nome visível ou posição na lista.
- Máscaras, caminhos, camadas auxiliares, dados arbitrários Mocha, botões de preset, opções
  do host, GPU e consultas temporais adicionais podem exigir interfaces ainda não implementadas.
- O importador preserva a árvore, os registros de animação e todos os bytes originais do FFX
  no comentário da camada, com undo/redo e persistência existentes. O log do diagnóstico
  inclui a árvore decodificada sem duplicar `sourceHex`; o FFX original não é alterado.

O conversor produziu 101 diagnósticos para este FFX: 85 propriedades sem mapeamento, 11
definições parciais, um valor incompleto, dois status parciais e dois avisos gerais. Contagens de propriedades do
editor referem-se apenas ao que foi aplicado, e não ao total decodificado.

## Descoberta e identidade do módulo AEX

A descoberta lê arquivos sem carregar DLLs. Mantém pastas configuradas, busca instalações
After Effects e `Adobe/Common/Plug-ins/7.0/MediaCore`, evita visitas duplicadas e não segue
symlinks. Limites: 64 pastas, profundidade 16, 20000 entradas, 4096 candidatos; a inspeção
completa de metadados é limitada a 256 MiB por arquivo e 1 GiB de leituras por varredura.

A nova leitura PE verifica AMD64/PE32+ DLL e extrai tabelas de importação/exportação,
importações atrasadas com RVA, versão fixa `VS_VERSION_INFO` e propriedades PiPL estruturadas.
A identidade PiPL deve declarar `kind=eFKT` e match name exato `S_Shake`; uma ocorrência de
texto em outra seção ou um nome de arquivo parecido não identifica o efeito. PiPL de outro
tipo de extensão é separado de um efeito. Formatos não reconhecidos permanecem desconhecidos.
A leitura PiPL foi testada com recursos sintéticos, não com um binário comercial Sapphire.

No carregamento explícito, a integração aexlo também passa a registrar os dados informados
por `PluginDataEntryFunction2`: nome, match name, categoria, tipo e entry point. Isso funciona
mesmo para o Gain de teste, que não possui PiPL. Depois de GLOBAL_SETUP, registra-se também
`my_version` como inteiro `PF_VERSION`, sem convertê-lo em uma versão comercial inventada.
O runtime continua escolhendo um único entry point; módulos que exportam vários efeitos
exigem validação adicional. Ausência de PiPL reconhecido não prova que o efeito está ausente.

## Runtime e diagnóstico implementados

O ABI permanece em um processo auxiliar Windows x64 separado. O host externo é aexlo
`28e9f9a4a371d0ecae0d2b183e167755b3a7cac3` (MIT), com duas correções Windows de FourCC
já existentes e o novo patch verificável `patch_native_host.py` para identidade, IDs e etapas.
Nenhum ABI nativo/unsafe foi adicionado ao workspace seguro do editor.

Checkpoints locais distinguem abertura/resolução de entry point, GLOBAL_SETUP, PARAMS_SETUP,
SEQUENCE_SETUP, verificação de capacidades, SMART_PRE_RENDER, SMART_RENDER e leitura de saída.
Uma etapa registrada antes da chamada indica a última operação iniciada; só os indicadores
confirmados e o frame validado provam conclusão. Saída anormal, erro de suite, timeout ou
falha de licença permanecem falhas, com a última evidência preservada.

O diagnóstico enumera nomes, índices, IDs persistentes, tipos, valores padrão atuais,
intervalos e opções quando fornecidos pelo plugin. Ele pode tentar renderizar **valores
padrão nativos** mesmo quando há tipos que o editor ainda não sabe expor. O editor continua
aceitando apenas sliders float; dados arbitrários e referências não têm mapeamento FFX.
Por isso um teste de defaults bem-sucedido não habilita automaticamente o preset S_Shake.

O teste usa um frame procedural original de 64×64, RGBA premultiplicado float32, exige Smart
Render e Float Color Aware, verifica tamanho e valores finitos e mede se os pixels mudaram.
A ferramenta conserva mensagens de suites indisponíveis, exit code e checkpoints. Prazo de
30 s por execução, logs/respostas de até 1 MiB e no máximo 16 alvos; o editor mantém seu
prazo de 10 s e limite de 4194304 pixels por frame. O tempo entregue ao plugin ainda é
quantizado a milissegundos; isso também precisa evoluir antes de prometer precisão temporal
geral do FFX no runtime nativo. Isolamento de processo não é sandbox do
Windows, nem proteção contra um módulo malicioso com as permissões do usuário.

Dependências: imports diretos e atrasados são relacionados a pesquisas limitadas na pasta do
plugin, aplicativo, Windows/System32 e PATH. `notFoundInLimitedSearch` é uma pista, não prova
de dependência ausente: API sets, dependências transitivas, side-by-side e LoadLibrary dinâmico
exigem evidência do loader local. O erro real de carregamento também fica no log.
Não há alteração de licenciamento, servidor de licença ou distribuição de DLL comercial.

## Como executar no Windows 11 x64

1. Extraia o ZIP inteiro em uma pasta gravável. Abra `Launch.cmd` para usar o editor.
2. Abra **`Diagnosticar-Sapphire.cmd`**. Ele testa o Gain incluído e módulos cujo PiPL
   declara exatamente S_Shake nas pastas de instalação encontradas. O log fica em
   **`diagnostico-sapphire.json`**, na pasta extraída. Não há upload automático.
3. Para incluir a análise do FFX, arraste `s shake.ffx` para esse CMD.
   Isso decodifica parâmetros/chaves, mas não os aplica automaticamente ao Sapphire.
4. Para outra pasta, ou um AEX sem PiPL reconhecido, use PowerShell nesta pasta:

```powershell
# Apenas inventário: não executa os plugins.
.\effectcraft-aex-diagnostic.exe --folder 'C:\CAMINHO\DA\PASTA' --out '.\inventario.json'

# Escolha um módulo instalado real a partir do inventário; não use um nome presumido.
.\effectcraft-aex-diagnostic.exe --plugin 'C:\CAMINHO\EXATO\MODULO.aex' --preset 'C:\CAMINHO\s shake.ffx' --out '.\teste-plugin.json'

# Repetir a prova do runtime próprio, sem instalar software de desenvolvimento.
.\effectcraft-aex-diagnostic.exe --self-test --out '.\teste-gain.json'
```

O caminho `MODULO.aex` acima é um placeholder, não uma identificação do módulo Sapphire.
O log mostra o match name declarado em runtime. Compartilhe esse JSON para orientar a
próxima etapa; ele contém caminhos locais e parâmetros, portanto pode ser revisado antes.
O resultado do programa salvar um log não significa que o plugin passou: consulte
`executions[].checkpoint`, `frame.verified`, `frame.pixelsChanged`, erros e limites.
`commercialCompatibilityVerified` permanece false nesta ferramenta.

## AEP e referência visual desta solicitação

O novo AEP produz 2 composições, 2 camadas e 13 propriedades mapeadas, com 105 diagnósticos.
Ambas as composições têm 1440×1440, 60 fps e duração aproximada de 31,95 s. O JPEG de origem
referenciado não foi anexado e está ausente neste ambiente. O efeito nativo e suas chaves
não foram aplicados pelo importador AEP: o original continua necessário para os dados não
mapeados. Isso é diferente de uma importação sem avisos ou fidelidade comprovada.

O MP4 de referência foi inspecionado com ffprobe como oráculo externo: H.264, sem stream de
áudio, 1440×1440, 60/1 fps, 31 frames, duração 0,516667 s. ffmpeg/ffprobe não são ligados,
incluídos ou distribuídos. O trecho de vídeo não é o projeto inteiro e não prova que o clock
do FFX deva ser substituído por 60 fps. Não há render Sapphire do EffectCraft neste ambiente
para calcular diferenças, SSIM/PSNR ou verificar equivalência das curvas. Preview e exportação
já passam pelo mesmo EffectPlugin no teste próprio; isso não valida a aparência do Sapphire.

## Validação e entrega

A entrega exige `cargo xtask ci` antes do commit e um workflow Windows MSVC bem-sucedido.
Além das regressões AEP/FFX e do teste nativo Gain já existentes, a embalagem exige a execução
do diagnóstico portátil: varredura sem execução, identidade exata por callback, ID persistente,
frames realmente alterados em dois tempos, preservação do FFX no log e checkpoint após saída
71 provocada pelo plugin de teste. Os resultados desta build ficam em `build.json`,
`validation/WINDOWS-VALIDATION.json`, logs e JSONs de evidência no ZIP. O relatório descreve
os critérios; o sucesso da execução deve ser conferido nesses registros da build publicada.
O teste Windows do editor também compara os pixels do Gain em preview com o frame realmente
exportado em PSD, admitindo apenas a quantização de 8 bits desse formato de saída.

O commit e30d8d6d, o importador Scale e todas as entregas anteriores são preservados. O ZIP
Windows contém editor, CLI, conversor, worker, diagnóstico, launcher e somente o Gain original
redistribuível. Código-fonte, scripts, dependências fixadas, licenças e histórico estão no ZIP
fonte. O uso da interface no Windows 11 e qualquer plugin comercial ainda precisam de teste
local. Carregar, inicializar, renderizar defaults, aplicar FFX e comprovar fidelidade são
resultados distintos e continuam registrados separadamente.
