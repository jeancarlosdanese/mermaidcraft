# FEAT: visual personalizável (estilo esboço)

Estado: implementado, aguardando revisão · 18/11/2026

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
- `classDef`, `class` e `style` do texto continuam valendo por cima de qualquer modo.
- O visual "clean" antigo (`--look clean`) continua igual: traço limpo, Liberation Sans e cores
  clássicas.

## Onde aparece

- **App:** menu Exibir › Aparência, com cada opção e "Restaurar padrão". A escolha fica nas
  preferências do app.
- **CLI:** `render` aceita `--font`, `--colors`, `--fill`, `--stroke` e `--arrows`, além do `--look`.
- **MCP:** a ferramenta `diagram_render` aceita os mesmos campos.

## Mudanças visuais junto com as opções

- Cores mais claras (tons 0–1 da paleta Open Color) e bordas no tom forte.
- Setas com ponta aberta em "V" e traço mais calmo que o contorno dos nós.
- Hachura feita no motor (sem dependência nova), com fundo claro por baixo para o texto continuar
  legível.
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

## Verificação (18/11/2026)

- `cargo clippy --workspace --all-targets -- -D warnings`: limpo.
- `cargo test --workspace`: tudo passando, com quatro testes novos (padrão = Proposta A; todas as
  combinações renderizam e o rótulo cabe na caixa em todas as fontes; cores por grupo e por forma;
  hachura determinística).
- `--look clean`: SVG idêntico ao da versão anterior nos três exemplos (só o atributo
  `font-family` passou a ter aspas).
- App aberto no modo de captura: canvas com Shantell Sans e as cores novas, sem erro.
- Não verificado: o menu Exibir › Aparência com cliques reais.
