# CortezEffects — redesign e personalização

Edição experimental Windows x64, baseada no compositor aberto EffectCraft. O código original,
as licenças, os créditos e todo o histórico de compatibilidade foram preservados. Esta edição
não é afiliada à Apple, Adobe ou Boris FX.

## Estado preservado

- Base do redesign: `c27cc0dd0bb12aeba465097af31e658e3b3ff080`.
- Ancestral obrigatório: `e30d8d6dc23af8d95c76072a6e693cbd42b398a9`.
- Branch: `feat/cortezeffects-liquid-glass`; nenhuma reescrita dos importadores ou do renderizador.
- Antes das alterações: `cargo xtask ci` aprovado, 193 suítes, 3.438 testes aprovados,
  zero falhas, 31 ignorados; 33 crates WebAssembly verificados.
- Os documentos históricos de AEP/FFX/S_Shake continuam em `packaging/after-effects/`.

## Identidade e componentes

O executável distribuído chama-se `CortezEffects.exe`. Título de janela, nome do produto,
descrição e nome original nos recursos VERSIONINFO, ícone, barra de tarefas, About, Home,
menus e mensagens de marca usam CortezEffects. Os pacotes Cargo e auxiliares permanecem com
seus identificadores anteriores para preservar comandos, APIs, automação e descoberta de runtimes.

A marca Ce foi adaptada com geração de imagem a partir da referência fornecida pelo usuário.
Os PNG de 256/512/1024 px e ICO multirresolução preservam a proporção. Sidecars e o índice
`ATTRIBUTION.md` registram a origem e a licença da contribuição. As atribuições do EffectCraft, ArtCraft, fontes e dependências permanecem disponíveis.

O design system egui/eframe existente recebeu superfícies azul-marinho/violeta, realce lavanda,
bordas discretas, reflexos, sombras e cantos configuráveis. O mesmo conjunto de tokens atende
Project, Effects & Presets, Effect Controls, Plugin Manager, Composition, Properties, Graph Editor,
timeline, exportação e diálogos. O indicador temporal ganha cor ciano e as chaves selecionadas
usam o destaque escolhido. Não houve alteração de tempos, valores ou interpolações.

O topo oferece acesso direto a Settings e Appearance & Personalization; a barra nativa do
Windows mantém minimizar, maximizar, fechar, redimensionamento e Snap. Em janelas largas,
o menu também mostra o projeto e o estado de salvamento. Home mantém New/Open Project,
templates, tutoriais e a lista real de projetos recentes, acrescentando a marca e personalização.

A splash aparece durante a preparação real das fontes nos frames de inicialização existentes.
Não há espera artificial. A criação do dispositivo gráfico que ocorre antes do primeiro frame
continua síncrona; não há indicador de progresso percentual inventado para essa fase.

## Personalização real e persistente

Abra o botão de brilho no topo ou Settings → Appearance & Personalization:

- Liquid Glass Dark (padrão), Liquid Glass Light, Midnight Purple, Graphite e Classic.
- Roxo, azul, rosa, ciano, vermelho, verde e entrada hexadecimal personalizada.
- Blur do fundo de vidro, opacidade de painéis, glow, reflexos, cantos, sombras e saturação.
- Wallpaper PNG/JPEG local: escolher, remover, opacidade, blur, escurecimento e Fill/Fit/Stretch.
- Inter ou JetBrains Mono (ambas OFL), escala de interface e escala de texto.
- Movimento reduzido, baixo consumo visual e redução de decoração durante playback/exportação.
- Restauração dos padrões de aparência.

As alterações são salvas automaticamente. Cancelar restaura e salva os valores anteriores ao
diálogo. As preferências usam o mesmo `prefs.json`, inclusive no Windows. Campos novos têm
defaults; os campos antigos, favoritos, projetos recentes, atalhos, diretórios AEX e campos
desconhecidos são mantidos. IDs de tema `dark`, `light` e `darker` continuam aceitos.

Esta entrega é um ZIP portátil. As receitas históricas de MSI/AppImage permanecem na árvore,
mas não foram usadas nem validadas para esta distribuição.

O modo portátil conserva `EffectCraftData`, `%APPDATA%\EffectCraft` e os caminhos de cache
legados, deliberadamente. O diretório `Documents/EffectCraft/Presets` e a substituição
`EC_PRESETS_DIR` também são mantidos. Não é preciso mover ou renomear preferências existentes. A imagem
de wallpaper é referenciada pelo caminho local: mover/apagar esse arquivo requer selecioná-lo
novamente; uma falha de leitura gera diagnóstico sem alterar o projeto. Para atualizar o conteúdo
de uma imagem no mesmo caminho, use Remover Wallpaper e Escolher Wallpaper para recarregar o cache.

## Desempenho e limites visuais

Não foi adicionada camada web ou dependência gráfica. Mica/Acrylic não são utilizados:
o backend atual pinta painéis internos em uma única superfície opaca, e adicionar o material
nativo não forneceria blur independente desses painéis. O acabamento é uma simulação portátil,
com reflexos/bordas por primitivas simples e o wallpaper previamente desfocado em cache.
Não há refração real, blur contínuo do desktop ou do conteúdo sob cada painel.

Diálogos usam preenchimento opaco para manter textos e diagnósticos legíveis, mesmo com
transparência alta nos painéis.

O wallpaper é decodificado em uma thread separada, com uma tarefa pendente no máximo,
arquivo limitado a 32 MiB, dimensões de entrada até 8192², orçamento de decodificação de
128 MiB e textura de até 1280². Decode/blur não são repetidos a cada frame. A textura anterior
é liberada ao trocar a imagem; falhas não são repetidas continuamente. A cor, enquadramento,
opacidade e escurecimento não exigem nova decodificação. O modo Classic/baixo consumo desliga
os reflexos, glow, sombras e blur. Decoração adicional é reduzida durante playback e renderização.

As microinterações dos botões usam easing, com repintura apenas durante a transição. Menus
utilizam o tempo de animação do egui. Movimento reduzido desliga essas transições e o spinner
da splash. A preferência de movimento do sistema Windows ainda não é consultada automaticamente.
Não há animação nova para arrastar/docar painéis, nem alteração do algoritmo de scroll/zoom da
timeline: o comportamento existente foi preservado. Escalas de texto grandes podem exigir
alargar painéis; não houve auditoria completa de acessibilidade.

O wallpaper só é desenhado pela interface. O pasteboard, a timeline e os pixels da composição
permanecem opacos. Preview, efeitos nativos e exportação mantêm os caminhos de processamento
anteriores. Não se afirma ganho de velocidade sem benchmark.

## Verificação desta edição

O CI local completo (`cargo xtask ci`) passou: 194 suítes, 3.445 testes aprovados, zero falhas
e 31 ignorados. Formatação, Clippy com warnings como erros, camadas, 36 assets atribuídos e
33 crates WebAssembly passaram. Os testes novos cobrem persistência real, migração, defaults,
clamps, temas, controles de personalização e Cancel. A troca de aparência preservou tanto o
JSON do projeto quanto os pixels RGBA renderizados no teste automatizado.

Foram renderizados e inspecionados snapshots reais dos cinco temas, Home, personalização,
wallpaper, Plugin Manager e escalas de interface de 150% e 200%. Os testes privados existentes
de Scale FFX com keyframes/pixels animados e preservação de AEP/S_Shake também passaram;
os arquivos privados não são distribuídos. Isso não comprova execução comercial do Sapphire.

O workflow Windows exige os testes reais de FFX, clocks, transformação Scale e processamento
do AEX Gain original, incluindo recuperação de falha do worker e exportação PSD. Também
executa os testes de personalização e geometria de janela e verifica os metadados VERSIONINFO
antes de empacotar. A Release só é publicada após sucesso desse job.

O ambiente de desenvolvimento Linux/headless não valida interação manual com o Windows 11,
DWM, barra de tarefas, Snap, monitores físicos nem mudanças de DPI em uma janela real. Os
testes de geometria/DPI são automatizados e não substituem esse teste manual. Plugins Sapphire
comerciais não estão instalados aqui. **S_Shake não foi declarado compatível:** binding do FFX
e execução comercial continuam pendentes. `Diagnosticar-Sapphire.cmd` e o relatório SAPPHIRE-S_SHAKE.md
permanecem no pacote, sem binários comerciais, vídeo, AEP ou FFX privados.

## Arquivos principais

- `assets/cortezeffects/`, `ATTRIBUTION.md`: marca e formatos do ícone.
- `crates/ui-egui/src/theme.rs`, `glass.rs`, `dock.rs`, `widgets.rs`: design system e fundo em cache.
- `header.rs`, `panels/home.rs`, `panels/settings.rs`, `panels/dialogs.rs`: marca e controles reais.
- `crates/engine/src/prefs.rs`: schema, validação e defaults compatíveis.
- `crates/ui-egui/src/i18n*`, menus públicos, `apps/effectcraft/src/main.rs`: identidade e traduções.
- `apps/effectcraft/build.rs`: ícone/recursos Windows; créditos originais mantidos.
- `crates/ui-egui/tests/ui_personalization.rs`: regressões de personalização.
- `.github/workflows/after-effects-downloads.yml`, `packaging/after-effects/package_downloads.py`:
  Windows x64, validação, ZIPs, source bundle de histórico e Release experimental.
- `README.md`, `packaging/windows/portable.txt`: instruções e contexto de distribuição.

## Instalação e teste no Windows 11 x64

Baixe `CortezEffects-Windows-x64.zip`, extraia completamente numa pasta gravável e abra
`Launch.cmd` ou `CortezEffects.exe`. Não há MSI nem necessidade de administrador. Copie
seu `EffectCraftData` anterior para junto do novo executável se quiser preservar o modo portátil.
Os executáveis experimentais não são assinados. O pacote não inclui plugins comerciais.

Teste seus projetos existentes, importe Scale e abra o Plugin Manager antes de alterar os temas.
Experimente o wallpaper e reinicie para conferir a persistência. Para Sapphire, execute
`Diagnosticar-Sapphire.cmd`; esse diagnóstico não promete aplicar o preset automaticamente.

A página pública de Releases do fork é
https://github.com/simoescortez1-star/effectcraft/releases.
O link específico e os checksums da nova compilação são informados após verificar os downloads.
