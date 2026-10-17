# CortezEffects — importação FFX e efeitos nativos

## Base e método

Esta etapa parte de `5c06fb7528235429be8392a0542047efdaa08a90`, preservando
o redesign CortezEffects e os ancestrais `c27cc0dd` e `e30d8d6d`.
Branch: `feat/ffx-native-effects`. O CI inicial passou com 194 suítes,
3.445 testes aprovados, zero falhas, 31 ignorados e 33 crates WebAssembly.
Os presets privados fornecidos pelo usuário não são distribuídos.
Nenhuma regra de leitura depende do nome de arquivo.

## Causas reproduzidas antes das mudanças

1. O leitor guardava apenas o último caminho `tdsp`. Presets com vários caminhos
   antes do sentinel associavam todos os blocos de dados ao último identificador.
   No arquivo Warp, os parâmetros de Tint eram atribuídos a `ADBE WRPMESH`.
2. O leitor exigia igualdade entre todos os relógios do contêiner e das propriedades.
   O arquivo combinado tem `beso = 30720` e descritores `tdb4 = 25600`.
   Era rejeitado no byte 676, antes da aplicação de Scale e Motion Tile.
3. O adaptador de efeitos mapeava somente um subconjunto de Gaussian Blur.
   Warp e Motion Tile já possuem implementações CPU no motor; suas identidades
   After Effects não eram resolvidas para essas implementações.
4. O registro de parâmetros tem diferenças de unidades e enumeração: o centro de
   Motion Tile é salvo normalizado e seus controles booleanos são números; Warp
   usa popups iniciados em 1 e a ordem Twist/Squeeze difere do registro interno.
   Copiar números sem conversões não seria suficiente.

## Dados efetivamente decodificados

### `corteiz pinch.ffx`

- 13.635 bytes; SHA-256 `b1685d3fd5f32ae3cda4cc05336f6e2f4ea020b7230a467a4087f05a927e1d2a`.
- Caminhos: `ADBE Effect Parade/ADBE Tint`, depois `ADBE Effect Parade/ADBE WRPMESH`.
- Relógio: 30.720 ticks/s nas propriedades examinadas.
- Tint: preto `[0,0,0,0]` e branco `[0,255,255,255]` nos quatro componentes salvos;
  Amount to Tint tem chaves 100 → 0 nos ticks 0 e 10.240 (0 e 1/3 s).
- Warp: Style 12, rotulado FishEye pela tabela salva; Axis 1, Horizontal.
  Bend tem chaves 12 → 0 nos ticks 0 e 1.024 (0 e 1/30 s).
  Horizontal Distortion e Vertical Distortion são zero.
- As quatro chaves de Tint/Bend têm códigos temporais 1/1.
- Bend contém uma expressão habilitada no arquivo. O texto é preservado e fica
  desativado ao importar, como nas edições anteriores. A animação por expressão
  não é comprovada por reconstruir apenas os keyframes.
- Há dados após o contêiner RIFX; permanecem preservados com os bytes originais,
  sem atribuir semântica não verificada.

### `scale in.ffx`

- 11.059 bytes; SHA-256 `f690ad5d406ca9ecfde06e7e48e69223738ab0d124c76b1cdedb3839fb1b8d1b`.
- Caminhos: `ADBE Transform Group/ADBE Scale`, depois `ADBE Effect Parade/ADBE Tile`.
- Contêiner: 30.720 ticks/s; propriedades: 25.600 ticks/s. O relógio de cada
  propriedade controla suas chaves; o relógio do contêiner é retido separadamente.
- Scale, três componentes salvos, em unidades percentuais após conversão:

| Tick | Segundos | Scale (%) | Códigos temporais |
|---|---|---|---|
| 0 | 0 | 200, 100, 0 | 2/2 |
| 5910 | 0,230859375 | 95, 100, 0 | 2/2 |
| 9671 | 0,3777734375 | 105, 100, 100 | 2/2 |
| 14507 | 0,5666796875 | 100, 100, 100 | 2/2 |

- As curvas contêm velocidades e influências por componente. Por exemplo, a
  influência de saída da primeira chave é 0,001; a de entrada da segunda é
  aproximadamente 0,59849286. Os dados completos permanecem na árvore preservada.
- Motion Tile: Center `[0.5,0.5]`, Tile Width/Height 100/100,
  Output Width/Height 410/390, Mirror Edges 1, Phase 0, Horizontal Phase Shift 0.
  Os parâmetros de Motion Tile são estáticos neste arquivo.
- Os valores Z zero nas duas primeiras chaves de Scale estão realmente salvos;
  não são substituídos por valores inventados.

## Correção inicial do parser

Os caminhos agora são associados aos blocos por uma fila, suportando a forma em
lote e a forma intercalada. A representação preserva caminhos completos, offsets,
tipos, hierarquia de parâmetros, valores, curvas, expressões, dados desconhecidos
e os bytes originais. Cada propriedade numérica registra seu próprio relógio.
Declarações conflitantes do mesmo relógio de contêiner continuam sendo rejeitadas;
diferenças legítimas entre propriedades deixam de ser tratadas como corrupção.

Nesta primeira etapa, 43 testes do adaptador externo passaram, incluindo três
novos testes gerados de estrutura, relógios, truncamento e dados pendentes.
Os dois arquivos reais foram decodificados novamente: as duas identidades do
primeiro ficaram corretas e as quatro chaves de Scale do segundo foram mapeadas.
Isso, isoladamente, não declara os efeitos aplicados nem a renderização validada.

## Referências

- Identidades de efeitos: [After Effects Scripting Guide — First-Party Effect Match Names](https://github.com/docsforadobe/after-effects-scripting-guide/blob/master/docs/matchnames/effects/firstparty.md).
- Parâmetros dos dois casos reais: tabelas `parT/pard` e registros de valores
  existentes nos próprios arquivos fornecidos, sem ler binários de plugins Adobe.
- Leitor externo fixado e proveniência: [arquitetura da edição](after-effects-compatibility.md).

Sapphire e outros AEX comerciais continuam fora desta tarefa de efeitos nativos.

## Pipeline funcional desta edição

`FaFX/RIFX → blocos binários → PropertyNode + bindings de caminhos → carrier/IR →
registro central de match names → conversões tipadas → reconstrução de chaves →
edição transacional da camada → renderizador CPU/GPU existente`.

O leitor continua em processo separado. Não há decodificador Adobe ou ABI AEX no
editor. Os overlays pertencem à edição e usam o leitor externo fixado descrito
na documentação de proveniência. AEP e FFX compartilham o modelo de propriedades,
as conversões numéricas e a reconstrução temporal/espacial quando aplicável.
O fallback anterior de Gaussian permanece disponível para suas tabelas de definição.

A IR retém os identificadores e caminhos completos, índices em ordem salva,
offsets, profundidade, tipos, descritores de parâmetros, valores, chaves, modos,
influências, velocidades, tangentes, expressões e bytes originais. O carrier só
contém instâncias renderizáveis para os mapeamentos aceitos. Dados desconhecidos
continuam na árvore e no `sourceHex`, em vez de criar efeitos fictícios.

## Registro nativo e extensibilidade

`crates/effects/src/after_effects.rs` centraliza os IDs e conversões. O comando
`anim.afterEffectsRegistry` expõe efeitos, parâmetros, defaults, limites, popups,
tipos, disponibilidade de render e condições das propriedades Transform.

| Identidade original | Implementação CortezEffects | Mapeamento explícito |
|---|---|---|
| `ADBE WRPMESH` | `ec.distort.warp` | Style, Axis, Bend, Horizontal/Vertical Distortion |
| `ADBE Tile` | `ec.stylize.motiontile` | Center, Tile Width/Height, Output Width/Height, Mirror Edges, Phase, Horizontal Phase Shift |
| `ADBE Tint` | `ec.color.tint` | Black, White, Amount |
| `ADBE Gaussian Blur 2` | `ec.blur.gaussian` | Blurriness, Dimensions, Repeat Edge |

O registro inventaria 28 efeitos prioritários, todos com implementação no motor:
os quatro acima, Transform, Corner Pin, Offset, Mirror, Polar Coordinates,
Turbulent Displace, Directional Blur, Radial Blur, Fast Box Blur, Sharpen,
Unsharp Mask, Curves, Levels, Exposure, Hue/Saturation, Fill, Brightness & Contrast,
Glow, Drop Shadow, Venetian Blinds, CC RepeTile, Gradient Ramp, Fractal Noise e
Posterize Time. **Existência no motor não significa compatibilidade FFX completa.**

Para os outros efeitos da lista, há resolução experimental por declaração salva:
o match name do efeito deve pertencer ao registro; o parâmetro deve ter ID próprio
com esse prefixo, declaração `pard` compatível, rótulo canônico único e tipo de
controle compatível. Popups exigem a mesma lista e ordem de opções. Não se adivinha
um índice pelo nome do arquivo. A declaração encontrada é registrada com seu ID
original nos diagnósticos. Rótulos localizados/ambíguos, controles arbitrários,
referências de outras camadas, curvas binárias personalizadas e dados sem valor
decodificado continuam não aplicados. Esse caminho é sempre marcado experimental
e parcial; não é promessa de importação de qualquer preset dos 28 efeitos.

Pontos normalizados são resolvidos no tamanho real de cada camada selecionada,
incluindo defaults nativos. No caso real, `[0.5,0.5]` tornou-se `[8,8]` numa fonte
16×16, independentemente do carrier 256×256. Cores usam a ordem ARGB salva e a
conversão de faixa; popups e booleanos mantêm tipos próprios, sem coerção genérica
para escalar. Seletores discretos usam Hold. Valores fora dos limites suportados
são preservados com motivo de rejeição, sem clamp silencioso no importador.

## Transformações, curvas e aplicação

Os 13 identificadores Transform cobrem Anchor Point, Position, Scale, Opacity,
Orientation, Rotate X/Y/Z, Skew, Skew Axis e Position_0/_1/_2. Rotation e Z Rotation
referem-se ao mesmo `ADBE Rotate Z`. Eixos separados reutilizam o mecanismo de
separação existente, mantendo horários independentes; a propriedade combinada
anterior permanece no projeto. Skew/Skew Axis são propriedades opcionais, criadas
apenas quando importadas, e passam pela matriz de renderização existente.
Projetos sem Skew usam o mesmo caminho numérico anterior.

Propriedades 3D requerem uma camada 3D. O FFX não muda esse switch por suposição;
o relatório informa a condição. Skew importado suporta -85 a 85 graus.
Um vetor 2D mantém o terceiro componente estático existente do destino.

Tempos usam `Tick` e o relógio da propriedade. As chaves são deslocadas pelo tempo
local da camada no playhead, sem rebase para a primeira chave nem arredondamento
para quadros. O motor recebe os valores, influências e velocidades convertidos;
propriedades vetoriais mantêm seus componentes e tangentes espaciais decodificadas.
Dados reconhecidos como não espaciais não são reinterpretados como uma curva
espacial apenas porque o widget usa um controle Point.

Códigos 1/2 usam Linear/Bezier com handles salvos; os casos de influência/velocidade
zero produzem easing pela curva. O código 3 tem mapeamento experimental para Hold,
exercitado por um contêiner original gerado, sem comprovação em um arquivo Hold
autorado no After Effects. Modos desconhecidos ou handles inválidos geram aviso
explícito da aproximação, com os dados brutos preservados. Flags de auto/roving não
são inferidas quando não verificadas. A separação de uma Position espacial já
animada reutiliza a aproximação por velocidades do comando existente; o original
combinado é mantido. Normalização de pontos espaciais e equivalência temporal sob
mudança de proporção da camada ainda requerem uma referência After Effects.

Cada importação altera as camadas em uma única etapa de undo. Efeitos suportados
mantêm a ordem entre si; efeitos pendentes guardam sua posição na IR. Os efeitos
da camada continuam antes de seu Transform, de acordo com o pipeline existente.
Propriedades animadas de Transform e de efeitos são reveladas na timeline, com
UIDs novos para os efeitos, e podem ser selecionadas no Graph Editor. O importador
agora mantém `next_id` depois do último ID usado, evitando reparos ao reabrir e
colisões com o próximo efeito criado. Os caches seguem a invalidação transacional.

## Renderização de Warp e Motion Tile

As duas implementações já existiam. O trabalho liga seus parâmetros ao FFX,
corrige as conversões e testa os pixels; Warp não foi substituído por Scale ou
Position. O modo FishEye do arquivo usa o índice nativo 11, com Bend animado.
Os quinze modos nativos de Warp permanecem disponíveis, sem afirmar que suas
fórmulas sejam visualmente idênticas às do Adobe After Effects.

Motion Tile conserva dimensões e offsets de expansão, repetição e Mirror Edges.
A amostragem espelhada agora repete também os texels de borda, evitando faixas
transparentes entre tiles de uma imagem opaca em expansões fracionárias. CPU e
shader WGSL receberam a mesma regra. O modo sem espelhamento mantém a amostragem
anterior; não foi comparado a uma referência Adobe nas bordas fracionárias.

Warp/Motion Tile rejeitam intermediários acima de 64 Mi pixels ou de 32.768 por
dimensão; a entrada é mantida e um diagnóstico é registrado. O dispositivo GPU
pode ter limites inferiores. Essa proteção não promete tempo real nem uma quota
de memória total: um intermediário RGBA32F de 64 Mi pixels ocupa cerca de 1 GiB.
Reduza a resolução de preview ou a expansão para fontes muito grandes. Não foram
publicados benchmarks de velocidade nem testes de VRAM em hardware Windows.

## Relatórios e dados preservados

O popup informa arquivo, contagens aplicadas e diagnósticos expansíveis. A seção
técnica lista transformações, efeitos, match names, conversões, valores, primeiras/
últimas chaves, parâmetros pendentes e motivos. O estado `applied` indica aplicação
ao modelo; `partial` sinaliza mapeamentos experimentais/incompletos. Disponibilidade
de renderização não é o mesmo que renderização observada: o importador não marca
`rendered` ou equivalência visual como verdade sem executar uma medição.

**Exportar JSON** usa `anim.exportAfterEffectsPresetReport`, com árvore original,
bytes preservados, aplicação, relatório do conversor, registro nativo e propriedades
atuais da camada, incluindo valores/curvas após resolver coordenadas e deslocar
tempos. A preservação fica no comentário da camada e sobrevive a salvar/reabrir.
O botão só aparece em relatórios que fornecem contexto FFX e o teste usa o hook
real de escolha de arquivo e a escrita pelo comando do motor.

Limites anteriores são mantidos: FFX até 1 MiB, metadados até 6 MiB, histórico de
até 8 MiB por camada e preservação até 64 MiB por comando, no máximo 16 destinos.
O carrier admite até 256 efeitos e 256 conversões de pontos; JSON exportado é
limitado a 16 MiB. Um formato inválido produz erro e mantém o projeto intacto.

## Validação final e entrega

Validação executada neste ambiente Linux, com o conversor externo reconstruído:

- `cargo xtask ci`: 194 suítes, **3.454 aprovados, zero falhas e 34 ignorados**;
  formatação, Clippy, camadas, atribuições e 33 crates WebAssembly aprovados.
- Conversor fixado + overlays atuais: 13 suítes, **43 aprovados, zero falhas,
  24 ignorados**. Os testes privados ignorados foram executados separadamente
  quando os respectivos arquivos estavam disponíveis.
- Os **25 contêineres FFX originais gerados** passaram por decodificação, aplicação,
  contagem/tempos de chaves, alteração de pixels, exportação PSD, salvar/reabrir e
  undo/redo. Incluem as combinações solicitadas, Bezier, Hold experimental,
  Transform/Glow declarados, propriedades 3D e importação parcial. São fixtures
  do protocolo, **não arquivos de referência autorados no After Effects**.
- Os **dois presets reais privados** passaram. Warp aplica Tint e FishEye em ordem,
  preserva quatro chaves e muda 196 pixels no teste isolado da deformação. O
  combinado preserva quatro chaves/curvas de Scale, converte Tile Center para a
  camada real e cobre 3.905 pixels contra 256 sem Motion Tile na composição 64×64.
  Ambos passaram também em salvar/reabrir, exportação e preservação dos bytes.
- As regressões anteriores dos FFX gerados, Scale privado, AEP privado e S_Shake
  privado passaram. O S_Shake segue preservado sem uma execução comercial provada.
- Dois testes focados compararam CPU e Vulkan via **llvmpipe**: os estilos Warp e
  a família de tiles, incluindo Motion Tile. Não representam GPU física Windows.
- Foram renderizados e inspecionados os relatórios, Effect Controls, timeline e
  Graph Editor dos dois arquivos reais sobre um gradiente original. As curvas e
  os controles importados aparecem na interface. O teste de UI do botão JSON e
  seu hook de gravação passou; relatórios sem contexto FFX não mostram esse botão.
- Scripts Python compilaram e `actionlint` aprovou o workflow de entrega.

O workflow Windows exige a matriz de 25 FFX gerados, as regressões AEP/FFX/AEX e
os testes de interface/personalização antes de empacotar e publicar. Os presets
privados não são enviados ao GitHub; portanto não são declarados testados no runner
Windows. O relatório empacotado acrescenta o commit e o JSON com os resultados
realmente executados naquele runner, e a Release contém somas SHA-256 dos arquivos.

Sem referência visual Adobe para estes dois presets, não há medição de equivalência
com After Effects. O teste manual em Windows 11 e em GPU física continua pendente.
A expressão de Bend no Warp é preservada/desativada; seu bounce por expressão não
foi reproduzido. Sapphire continua uma tarefa separada e não está declarado compatível.

## Arquivos modificados por área

- Parser/IR: `converter-overlay/reader_compat.rs`, `apply_overlay.py`, `preset_reader_tests.rs`.
- Adaptador/conversões: `adapter_compat.rs`, `native_effects.rs`, `converter-Cargo.lock`.
- Registro: `crates/effects/src/after_effects.rs`, `lib.rs`.
- Aplicação/IDs/relatório: `crates/engine/src/commands/after_effects.rs`, `ffx_support.rs`, `link.rs`.
- Transformações opcionais: `crates/render/src/eval.rs`.
- Warp/Motion Tile: `crates/effects/src/distort3.rs`, `stylize2.rs`; `crates/gpu/src/fx_warp.rs`, `fx_stylize.rs`, shader `fx_stylize.wgsl`.
- UI de relatório: `panels/forms.rs`, `panels/dialogs.rs`, `menus.rs`, `i18n/ui.rs`.
- Regressões: `tests_ffx_native.rs`, `ui_after_effects.rs`, geradores originais de fixtures.
- Entrega: workflow `after-effects-downloads.yml`, `validate_windows.py`, `package_downloads.py`, README e estes documentos.

## Teste no Windows 11 x64

Extraia todo o ZIP portátil e abra `Launch.cmd` ou `CortezEffects.exe`. Crie uma
composição e selecione uma camada de imagem/vídeo. Posicione o playhead e use
Animation → Apply After Effects Preset. Confira Effect Controls e as propriedades
reveladas na timeline; selecione-as no Graph Editor para inspecionar as curvas.

No Warp real, Bend vai de 12 a 0 em 1/30 s e Tint vai de 100 a 0 em 1/3 s, após
o playhead. A expressão não é executada. No combinado, Scale tem quatro chaves e
Motion Tile expande 410% × 390%, com Mirror Edges ativo. Exporte o diagnóstico pelo
popup, salve/reabra o projeto e compare preview/exportação. O relatório da build
contém o commit, os testes Windows executados e o endereço da Release publicado.
