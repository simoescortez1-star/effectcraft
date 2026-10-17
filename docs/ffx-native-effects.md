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
