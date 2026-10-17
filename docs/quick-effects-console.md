# CortezEffects — Quick Effects Console

## Base e implementação independente

Esta edição parte de `dfbc9ef22108f46a78b605bee8ae8d137ecabcae`, mantendo
o importador FFX nativo, o redesign CortezEffects, AEP, AEX, timeline, Graph Editor,
renderização e exportação. Os ancestrais `e30d8d6d`, `c27cc0dd` e `5c06fb75`
continuam no histórico. Não foi reescrito o editor.

O ZIP privado fornecido tem 1.744.121 bytes e SHA-256
`bce02282da59571f8c46ec48a450b313a54a0f1ff80e6bab9ab361d9eee860e9`.
A listagem contém `FXConsole_1.0.7_Installer_x64.exe` (3.163.256 bytes).
Seu cabeçalho PE declara a máquina AMD64 (`0x8664`). Isso identifica a arquitetura
declarada do instalador; não comprova sua versão interna ou autenticidade.
O instalador não foi executado, instalado, descompilado ou usado para obter
código, ícones, interfaces ou outros recursos. Não entra nos ZIPs nem no repositório.

A [página oficial de apresentação da Video Copilot](https://www.videocopilot.net/blog/2016/10/new-workflow-plug-in-fx-console-is-now-available/)
documenta busca de efeitos/presets, atalhos e exportação de snapshots PNG.
Foi consultada como referência funcional pública. O comportamento específico do
instalador 1.0.7 não foi observado em execução. O código e a interface desta edição
usam os componentes e o design system do CortezEffects; não carregam o FX Console.

## Busca e aplicação

1. Selecione uma camada na composição.
2. Pressione **Ctrl + Espaço**.
3. Digite o nome do efeito, categoria, ID interno ou apelido.
4. Use **↑ / ↓** e **Enter**, ou clique no resultado.
5. **Esc** fecha o console e devolve o teclado ao editor.

O campo recebe foco ao abrir. A navegação não usa o atalho global do Windows e
não inicia o preview pela liberação de Espaço. Timeline e Composition usam o
mesmo dispatcher de atalhos existente. A composição de texto por IME é respeitada.
O atalho de abertura segue a proteção existente de campos de texto e menus abertos;
conclua a edição do campo antes de abrir o console.

O catálogo vem de `effectcraft_effects::all()`, incluindo efeitos registrados
posteriormente pelo sistema de plugins do editor. Não há uma segunda tabela de
implementações ou efeitos fictícios. O console consulta mudanças de identidade
do registro e atualiza seu catálogo. Efeitos AEX apenas descobertos em disco não
viram efeitos disponíveis por esse fato; continuam sob o Plugin Manager existente.

A busca normaliza caixa, pontuação, espaços e acentos comuns; reconhece o nome
original, o nome traduzido disponível, categoria, ID e apelidos salvos. Tolera uma
inserção, remoção ou substituição em palavras de pelo menos quatro caracteres.
Nome exato precede prefixos e aproximações; favoritos, frequência e recência ajudam
no desempate. O catálogo e os resultados são armazenados em cache; digitar não
lê arquivos FFX, não renderiza miniaturas e não interrompe a reprodução.

A aplicação chama `effect.apply` e a revelação existente de Effect Controls.
A criação da instância, UIDs, atualização do modelo, cache, preview e Undo/Redo
seguem o motor. Sem camada selecionada, o console informa o problema e fica aberto.
Não há aplicação quando a busca não tem resultados. O console fecha após sucesso.

Efeitos com defaults neutros continuam neutros: por exemplo, Warp com Bend zero
ou blur com raio zero. O console preserva esses defaults; não os modifica para
forçar uma diferença visual. Ajuste os parâmetros no Effect Controls. O teste
de pixels usa Tint sobre uma fonte colorida para comprovar processamento real.

## Favoritos, apelidos e atalhos

A estrela adiciona/remove favoritos e compartilha o estado com Effects & Presets.
Os favoritos antigos desse painel são reconhecidos e migrados ao usar o console.
Use **Apelido…**, escreva palavras próprias e salve para pesquisar um efeito ou
preset por esse texto. Use os filtros All, Effects, Presets, Favorites e Recent.

Histórico e frequência registram aplicações feitas pelo console. A reaplicação,
Undo e Redo não são confundidos: desfazer restaura o projeto; o histórico de uso
permanece uma preferência do usuário. Há limites de 256 favoritos, 40 recentes,
512 contadores e 256 apelidos para manter os dados de configuração limitados.

**Atalho…** em um efeito abre o editor de atalhos existente, filtrado pelo efeito.
Ele permite atribuir uma combinação ao `effect.apply` correspondente, com a mesma
detecção de conflitos dos outros comandos. Atribuições individuais a presets de
arquivo não foram adicionadas nesta etapa.

## Presets

O console inclui os presets internos de texto/animação, arquivos `.ecpreset`
salvos pelo usuário e arquivos `.ffx` nas pastas indexadas ou nos recentes.
Usa `layer.applyTextPreset`, `anim.applyBuiltinPreset`, `anim.applyPreset` e
`anim.applyAfterEffectsPreset`. **Não contém outro parser FFX.**

A pasta padrão permanece `Documents/EffectCraft/Presets`, ou `EC_PRESETS_DIR`
quando configurado, por compatibilidade com a edição anterior. Há uma pasta
adicional configurável em Settings → Quick Effects Console. A indexação ocorre
em um worker, usa nomes/extensões e não abre o conteúdo dos presets. Atualiza ao
abrir, a cada cinco segundos enquanto visível e com Refresh.

São visitados até quatro níveis de subpastas, 10.000 entradas e 2.000 presets.
Links de diretórios encontrados durante a busca não são seguidos. Diretórios
inacessíveis e limites produzem diagnósticos; não bloqueiam a busca de efeitos.
Para uma biblioteca mais profunda, configure diretamente a subpasta desejada.

**Aplicar FFX…** usa o seletor e o importador existentes. Os FFX aplicados a partir
desta edição entram nos recentes persistentes. FFX aplicados em edições anteriores
podem ser reencontrados pela pasta configurada ou selecionados novamente; não se
varrem comentários grandes de todos os projetos durante a digitação.

A presença de um FFX na lista significa identificação do arquivo, não aprovação
de compatibilidade. Ao aplicar, o relatório original informa propriedades/efeitos
aplicados, dados preservados e limites, com Exportar JSON. Um resultado sem
propriedades/efeitos aplicados não é registrado como aplicação funcional pelo console.
Expressões importadas continuam preservadas/desativadas conforme o importador.

Presets `.ecpreset` vazios ou sem qualquer item suportado não são contabilizados
como aplicação funcional. Propriedades omitidas e efeitos sem registro recebem
um relatório nativo. Efeitos indisponíveis permanecem preservados no modelo;
quando há itens compatíveis no mesmo preset, esses itens continuam aplicados.


## Aparência e configurações

Settings → **Quick Effects Console** permite habilitar/desabilitar a abertura,
personalizar o atalho pelo editor existente, ajustar largura, opacidade, quantidade
de resultados (4–30), favoritos primeiro, recentes, animação, pastas e escala de captura.
Preferências, favoritos, apelidos e uso são salvos pelo armazenamento existente.
Arquivos de configuração e projetos anteriores permanecem compatíveis.

A janela flutuante usa os tokens do tema, cores de destaque, superfície translúcida,
cantos arredondados, sombra e animação curta de abertura. Respeita Reduce Motion e
os temas existentes. Não muda o layout dos painéis principais. O vidro segue o
design system existente: reutiliza a textura de wallpaper já desfocada em cache
e reflexos leves. Não realiza blur dinâmico dos painéis/vídeo atrás do popup nem
usa Mica/Acrylic. Não utiliza APIs privadas ou atalhos globais do Windows.

## Captura de quadro

**Capturar PNG** e **Copiar quadro** usam `comp.saveFrameAs` e `comp.copyFrame`.
A fonte é o frame renderizado da composição no tempo atual, incluindo os efeitos;
não é uma captura da janela do editor. A saída existente é RGBA de 8 bits sobre
o fundo da composição, com o mesmo processamento do comando de frame do motor.

Sem pasta de captura, o seletor pergunta onde salvar. Com uma pasta configurada,
o nome incorpora o nome sanitizado da composição e um sufixo único; novas capturas
não sobrescrevem intencionalmente a anterior. A escala é de 25% a 100%, com guarda
de 16 megapixels por captura. Resoluções maiores exigem reduzir essa escala.

Os dois comandos também aparecem no editor de atalhos como Console: Capture…,
sem combinações reservadas por padrão. A integração de clipboard usa o caminho
egui/desktop existente. Capturar uma composição pesada pode demorar: este comando
explícito ainda usa a renderização síncrona de frames existente. A busca não a
executa em segundo plano a cada tecla. Não foi acrescentada uma galeria de imagens.

## Comandos e arquivos

Comandos de UI/control channel: `quickEffects.open`, `quickEffects.close`, `quickEffects.results`,
`quickEffects.apply`, `quickEffects.favorite`, `quickEffects.alias`,
`quickEffects.refresh`, `quickEffects.shortcut`, `quickEffects.importFfx`,
`quickEffects.capture` e `quickEffects.copyFrame`.

O estado persistente usa `quickEffects.*` no `prefs.json`. O atalho de abertura
usa o mesmo `shortcuts.json` e os presets de teclado anteriores; não há uma segunda
configuração de teclas que possa entrar em desacordo com o editor.

Arquivos principais: `crates/engine/src/quick_effects.rs`, `prefs.rs`,
`crates/ui-egui/src/panels/quick_effects.rs`, `menus.rs`, `ui_quick_effects.rs`.
O importador FFX registra o caminho recente após a importação. O roteador do menu
Recent Animation Presets agora encaminha extensões FFX ao conversor existente,
incluindo maiúsculas. O parser, os mapeamentos e as curvas anteriores permanecem
em uso; arquivos .ecpreset continuam no caminho nativo.

## Validação e entrega

Validação executada nesta etapa, em Linux:

- `cargo xtask ci`: **195 suítes, 3.465 testes aprovados, zero falhas**;
  35 testes dependentes de ambiente ignorados no conjunto padrão; 33 crates
  verificados para WebAssembly e 36 assets com atribuição verificada.
- Clippy completo com `-D warnings` aprovado, além das verificações de formatação,
  camadas, sintaxe Python e workflow com actionlint.
- Oito testes de UI do console aprovados: atalhos, foco, teclado, IME, aplicação
  real de Tint com comparação de pixels e Undo/Redo, presets nativos, diagnóstico
  parcial, favoritos, aliases, persistência após reinício, temas/DPI e captura
  PNG/saída de clipboard. Os testes de captura comparam a imagem ao renderer.
- O teste de integração FFX normalmente ignorado foi executado separadamente com
  o conversor real e um fixture de protocolo original gerado: importação pelo
  console, relatório preservado e reaplicação pelo menu de presets recentes
  aprovadas. Esse fixture não foi produzido pelo After Effects.
- Sete regressões adicionais de preservação aprovadas, incluindo os arquivos
  privados de Scale, AEP e S_Shake das etapas anteriores, os dois presets privados
  de Warp e Scale + Motion Tile, e a matriz de 25 casos FFX gerados. Os arquivos
  privados não entram no repositório nem nos pacotes. Esses testes preservam os
  diagnósticos de S_Shake indisponível; não comprovam execução do Sapphire.
- Capturas reais da UI headless inspecionadas nos temas Dark, Light e Midnight:
  busca, seleção por teclado, favoritos/apelidos, Effect Controls após Tint e
  configurações. Inspeção adicional em 1120 × 700 pontos a 175% de DPI; os testes
  automatizados também cobrem escalas de 100%, 150% e 200%.

O workflow Windows exige novamente os oito testes de UI, a aplicação FFX pelo
console, a matriz FFX e as regressões AEX originais antes de publicar os pacotes.
Os resultados específicos dessa compilação são acrescentados ao relatório de
entrega e incluídos em `validation/WINDOWS-VALIDATION.json` no ZIP Windows.

Teste interativo no Windows 11, clipboard físico do sistema e benchmarks em hardware
do usuário continuam pendentes. A build e os testes automatizados Windows serão
identificados pelo commit e pelo JSON de validação acrescentado ao pacote.
