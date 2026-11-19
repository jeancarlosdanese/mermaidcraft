# FEAT: visual personalizável (estilo esboço)

Estado: entregue na 0.2.0 (18/11/2026)

## Objetivo

Deixar o desenho dos diagramas no estilo quadro branco (traço à mão, cores claras, fonte
manuscrita legível) e permitir que cada pessoa ajuste esse visual. O padrão é a **Proposta A**
do protótipo: Shantell Sans, cores por grupo, preenchimento sólido, traço "artista" e setas em
grafite com ponta aberta.

Protótipo de referência: artifact "Protótipo MermaidCraft" (rough.js, só para avaliação).

## Opções (motor: `RenderOptions`)

| Opção | Valores | Padrão |
|---|---|---|
| Fonte | Shantell Sans, Patrick Hand, Kalam, Architects Daughter, Liberation Sans | Shantell Sans |
| Cores | por grupo, por forma, neutras, uma por nó, clássicas (Mermaid) | por grupo |
| Preenchimento | sólido, hachura, cruzado | sólido |
| Traço | limpo, artista, cartunista | artista |
| Setas | grafite, cor da origem | grafite |

- **Por grupo:** os nós de um subgraph usam a cor do grupo; os nós soltos seguem o papel da forma.
- **Por forma:** decisão em amarelo, início e fim (pílula, círculo) em verde, banco de dados em roxo,
  sub-rotina em azul, entrada e saída (paralelogramo, trapézio) em verde-água, hexágono em laranja,
  o resto em cinza.
- Em sequence e ER não há grupos nem formas: "por grupo" e "por forma" dão uma cor por participante
  ou entidade.
- `classDef`, `class` e `style` do texto continuam valendo por cima de qualquer modo.
- "Setas" vale para flowchart, sequence e ER.
- O visual "clean" antigo (`--look clean`) continua igual: traço limpo, Liberation Sans e cores
  clássicas.

## Onde aparece

- **App:** menu Exibir › Aparência, com cada opção e "Restaurar padrão". A escolha fica nas
  preferências do app. Quem tinha desligado o visual antigo (`sketch=0` ou `colorful=0` no arquivo
  de preferências) passa a ter traço limpo ou cores clássicas, em vez do padrão novo.
- **CLI:** `render` aceita `--font`, `--colors`, `--fill`, `--stroke` e `--arrows`, além do `--look`.
- **MCP:** a ferramenta `diagram_render` aceita os mesmos campos.

## Mudanças visuais junto com as opções

- Cores mais claras (tons 0–1 da paleta Open Color) e bordas no tom forte.
- Setas com ponta aberta em "V" em todos os modos de cor, menos o clássico (que mantém a ponta
  cheia), e traço mais calmo que o contorno dos nós.
- Hachura feita no motor (sem dependência nova), com fundo claro por baixo para o texto continuar
  legível. O custo é limitado: até 20 mil linhas por diagrama, espaçamento maior em formas muito
  grandes, e as linhas não passam de novo pelo traço à mão.
- Título do grupo no canto superior esquerdo, fora do caminho das setas que entram no grupo.

## Fontes

Todas com licença SIL OFL 1.1, embutidas em `assets/fonts/` com o texto da licença:
Shantell Sans 1.011 (instâncias estáticas Regular e Bold geradas da fonte variável oficial),
Patrick Hand, Kalam (Regular e Bold) e Architects Daughter (Google Fonts). As fontes sem negrito
usam o regular nos títulos.

## Fora desta entrega

- Correções de layout (setas atravessando nós, arestas paralelas, quebra automática de rótulo).
- Interface nova (barra flutuante, painel de estilo, seleção ligada ao texto).
- Diagramas em tema escuro.
- Guardar o visual no próprio arquivo (frontmatter `config: look: handDrawn`).

## Critérios de aceite

1. Sem preferência salva, o app e o CLI desenham com a Proposta A.
2. Cada opção muda o desenho no app, no SVG, no PNG e no PDF da mesma forma.
3. A medida do texto usa a fonte escolhida: o rótulo não vaza da caixa em nenhuma fonte.
4. `--look clean` gera o mesmo resultado de antes.
5. `cargo test` e `cargo clippy -- -D warnings` limpos.

## Revisão (18/11/2026)

Primeira passada do revisor sobre `8ff3684`: um achado médio (custo da hachura sem limite com entrada
hostil: 347 MB de SVG com 100 notas enormes) e seis baixos (ponta aberta × documento, `--arrows` sem
efeito no ER, licenças fora do `.app`, código sem uso, preferências antigas ignoradas, duplicação).
Todos corrigidos na mesma branch, com testes de regressão para a hachura e para as setas do ER.

## Verificação (18/11/2026)

- `cargo clippy --workspace --all-targets -- -D warnings`: limpo.
- `cargo test --workspace`: tudo passando, com quatro testes novos (padrão = Proposta A; todas as
  combinações renderizam e o rótulo cabe na caixa em todas as fontes; cores por grupo e por forma;
  hachura determinística).
- `--look clean`: SVG idêntico ao da versão anterior nos três exemplos (só o atributo
  `font-family` passou a ter aspas).
- App aberto no modo de captura: canvas com Shantell Sans e as cores novas, sem erro.
- Não verificado: o menu Exibir › Aparência com cliques reais.
