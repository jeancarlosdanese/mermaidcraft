# FEAT: interface do editor (fase C)

Estado: entregue na 0.5.0 (19/11/2026); ajustes na 0.6.1 e correções na 0.6.2

## Objetivo

Levar para o app a interface do protótipo aprovado na 0.2.0: o diagrama ocupa o centro, as
ferramentas flutuam sobre ele, o nó selecionado ganha um painel de estilo, e o texto e o desenho
andam juntos. Também desenhar o diagrama no tema escuro quando o app está escuro.

## O que muda

1. **Canvas com grade de pontos.** Fundo de papel com pontos discretos, que acompanham o zoom e o
   deslocamento. Ajuda a perceber a escala e o movimento.
2. **Ferramentas flutuantes.** A barra (Selecionar, Ligar, + Nó, Organizar, direção, desfazer,
   refazer e zoom) sai do topo e vira uma barra flutuante sobre o canvas, no alto e ao centro.
3. **Painel de estilo.** Com um nó selecionado, um painel flutua à direita do canvas: rótulo, forma,
   cor (automática ou um dos oito tons da paleta), contorno (sólido ou tracejado) e apagar.
   Com uma seta selecionada: rótulo, linha, ponta e apagar. Sem seleção, o painel some e a dica
   fica numa linha discreta no pé do canvas.
4. **Cada mudança vira texto.** Cor e contorno são gravados na linha `style <id> ...` própria do nó.
   Se ela já existe, é trocada no mesmo lugar; se não existe, entra antes do comentário
   `%% mermaidcraft:layout` (ou no fim). Propriedades que o painel não controla, como `color`, são
   mantidas. "Automática" remove `fill` e `stroke`; sem nenhuma propriedade, a linha some. Linhas
   compartilhadas (`style A,B ...`) e classes (`class`/`classDef`) não são reescritas; quando parte
   do estilo vem delas, o painel avisa.
5. **Linha que mudou pisca.** Depois de uma edição feita pelo desenho, as linhas alteradas do texto
   ficam realçadas por pouco mais de um segundo.
6. **Seleção ligada nos dois sentidos.** Selecionar um nó realça a linha que dá forma e rótulo a ele
   (ou a primeira menção, se ele não tem definição); uma seta realça a linha dela. O texto rola só
   quando a linha está fora da vista. Pôr o cursor no texto seleciona o que está sob ele: o id de um
   nó, o traço de uma seta, ou o primeiro nó da instrução quando o cursor está fora de um nome.
7. **Diagrama escuro.** Com o tema escuro do app, o canvas mostra o diagrama com as cores invertidas
   e o matiz preservado (o mesmo truque do Excalidraw: inverter 93% e girar o matiz 180°). Só a tela
   muda; SVG, PNG e PDF exportados continuam claros.

## Fora do escopo

- Tema escuro nos arquivos exportados e no CLI.
- Painel de estilo para grupos e para diagramas de sequência e ER.

## Critérios de aceite

1. `edit::set_node_style` grava, troca e remove a linha `style` do nó, sem mexer em outras linhas,
   e mantém propriedades que não foram alteradas (testes no motor).
2. Escolher uma cor no painel muda o nó no desenho e no texto, num único passo de desfazer.
3. Clicar num nó realça e mostra a linha dele no texto; pôr o cursor numa linha seleciona o nó.
4. No tema escuro, o canvas mostra o diagrama escuro e a exportação continua clara.
5. Capturas do app (modo `--screenshot`) em claro e escuro, com e sem seleção.
6. `cargo test` e `cargo clippy -- -D warnings` limpos.

## Verificação

- Testes do motor: `node_style_is_written_replaced_and_removed` (troca no lugar, linha nova antes do
  comentário de layout, remoção da linha inteira, duas linhas viram uma, CRLF mantido, valores
  inválidos recusados), `text_place_names_the_node_or_edge_under_it` e
  `dark_keeps_hue_and_flips_lightness`. No app: `changed_lines_cover_edits_inserts_and_deletes`.
- O desenho exportado é idêntico byte a byte ao da 0.4.0 nos exemplos e nos fluxos do HyVox.
- Capturas com `--screenshot`, em claro e escuro, com e sem seleção. Para isso o modo de captura
  aceita `MERMAIDCRAFT_SHOT_SELECT=<id>` e `MERMAIDCRAFT_SHOT_DARK=1`, lidos só nesse modo.

## Revisão (18/11/2026)

Revisor independente. Corrigidos:

- Alta e média (três passadas): o rótulo digitado no painel se perdia, ou ia para outro nó ou seta,
  conforme o clique seguinte (canvas, outro nó, texto, barra, arrastar, apagar, desfazer), porque a
  gravação dependia da ordem em que as partes da tela são desenhadas. Agora há uma regra só
  (`commit_label`): o painel guarda para qual nó ou seta o rótulo está sendo digitado e grava nele
  quando o campo perde o foco, quando a seleção muda, e antes de apagar, desfazer, refazer ou criar
  um nó. Enquanto há rótulo por gravar, o campo não é sobrescrito. Esc descarta o que foi digitado.
- Média: o texto ligava a seleção pela primeira menção do nó, e só uma das seis setas do exemplo
  podia ser escolhida pelo texto. Agora vale o que está sob o cursor (`edit::item_at`), e o realce
  usa a linha da definição. Id de grupo não é tomado por nó, e um traço que desenha várias setas
  (`A & B --> C`) não escolhe nenhuma delas.
- Média: a rolagem recentrava o texto a cada seleção, também na horizontal. Agora rola só quando
  a linha está fora da vista, e pelo lado dos números.
- Baixas: o painel copiava para a linha do nó as propriedades de linhas compartilhadas
  (`own_node_style`); a linha `style` ia para o fim do arquivo, depois do comentário de layout, e
  piscavam linhas que não mudaram; estilo vindo de `class` ficava invisível (agora há aviso); o doc
  falava em três opções de contorno; `hex` duplicado; a dica dos outros tipos de diagrama tinha
  sumido; o primeiro quadro saía sem a barra de ferramentas.
- Baixa, anterior à branch: gravar o rótulo de uma seta de traço compartilhado (`A & B --> C`) move
  a seta para uma linha própria e muda a numeração; a seleção agora acompanha a mesma seta (antes,
  "Apagar" logo depois apagava a outra).

## Limites conhecidos

- Um valor com `;` ou `,` entre aspas numa linha `style` própria impede o painel de reescrevê-la (a
  troca é recusada com aviso).
- O painel ainda cobre o canto direito do diagrama quando nada embaixo dele está selecionado; o
  ajuste à janela não reserva espaço para ele.
- A interação com o mouse (clicar, arrastar, digitar no painel) foi conferida pelo código e pelas
  capturas, não com cliques de verdade.

## Ajustes da 0.6.1

- **`;` sobrando.** Ao unir duas instruções `style` do mesmo nó escritas numa linha só
  (`style A fill:#fff; style A stroke:#000`), o `;` entre elas ficava para trás. Agora sai junto com
  a instrução removida, seja o que vem depois dela, seja o que vem antes quando ela fecha a linha
  (`A --> B; style A ...`), mesmo com um comentário `%%` no fim; a linha some quando não sobra nada
  nela. É a mesma limpeza que apagar um nó já usava (`clean_markers`), que passou a respeitar o
  comentário no fim da linha.
- **Painel sobre o nó selecionado.** Um nó ou seta selecionado embaixo do painel de estilo faz a
  vista deslizar para a esquerda até ele ficar visível, a 16 px da borda do painel, sem passar da
  borda esquerda do canvas. Acontece uma vez por seleção, nunca durante um arraste nem enquanto se
  digita (no texto, cada letra de um id é uma seleção nova); depois disso a vista não volta sozinha
  ("Ajustar à janela" refaz o enquadramento). Seleções que já estão à vista não mexem em nada.
- Revisão independente: a primeira versão tinha uma segunda rotina de limpeza do `;` (trocada pela
  que já existia), media o painel no quadro em que ele ainda era só estimado e deslizava aos trancos
  enquanto se digitava um id no texto. Corrigidos.

## Correções da 0.6.2

- **Arrastar com Shift não ligava nada.** Ao soltar, a conferência de "mover nó" esvaziava o
  arraste antes da conferência de "ligar", e a ligação se perdia: arrastar com Shift (ou com a
  ferramenta Ligar) de um nó a outro, ou a um espaço vazio para criar um nó ligado, não fazia nada.
  Vinha da 0.4.0.
- **Duplo clique ao lado do painel.** Na 0.6.1, o primeiro clique num nó perto do painel de estilo
  fazia a vista deslizar antes do segundo clique, e o duplo clique não abria o rótulo. A vista agora
  espera passar o tempo de um duplo clique (e o botão ser solto) antes de deslizar.
- **Testes de gestos.** `src/app.rs` ganhou testes que rodam o app quadro a quadro, sem janela, com
  cliques e arrastes sintéticos: selecionar, ligar pela ferramenta, ligar arrastando com Shift (a
  outro nó e ao vazio), mover um nó, duplo clique para editar o rótulo e "+ Nó" seguido de
  digitação. Os testes partem das preferências padrão, nunca do `settings.txt` de quem roda.
