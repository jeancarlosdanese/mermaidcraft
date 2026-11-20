# FEAT: grupos (`subgraph`) no canvas

Estado: fases A e B implementadas na branch `feat/grupos`; fase B aguardando o "pode commitar" (0.8.0)

## Problema

Os grupos (`subgraph`) só se editam pelo texto. No canvas dá para arrastar um grupo pelo título
(0.4.0), mas não dá para selecioná-lo, renomeá-lo, colori-lo, criar um grupo, pôr ou tirar um nó
dele, nem desfazer o grupo. O botão direito sobre um grupo abre o menu do espaço vazio, e o "Novo
nó aqui" cai fora do grupo (regra 10 da 0.7.0). O painel de estilo não vale para grupos, e uma
linha `style G fill:...` no texto não muda o desenho da moldura. Além disso, uma seta pode passar
por cima do título do grupo (no `fluxo-longo.mmd`, a que chega em "5. Envio").

## Objetivo

O grupo vira um componente como o nó: seleciona com um clique, tem menu de botão direito, painel
de estilo e teclado. Cada ação reescreve o texto com o motor, como as outras. E nenhuma seta passa
sobre o título de um grupo.

KRs:

1. Tudo o que o texto faz com um grupo (criar, renomear, pôr e tirar nó, desfazer, apagar, cor e
   contorno) dá para fazer pelo canvas.
2. Toda edição nova no motor tem teste em `crates/engine/tests/behaviour.rs`, e todo gesto novo
   tem teste sem janela (`mod gestures`).
3. Os testes da 0.6.2 e da 0.7.0 passam sem mudança, salvo o da regra 10 da 0.7.0, que muda de
   propósito (ver regra 6).

## Escopo

**Must:**

| Onde | Itens |
|---|---|
| Grupo (menu) | Renomear (F2), Cor ▸, Contorno ▸, Novo nó aqui, Desagrupar, Apagar com o conteúdo (Del) |
| Nó (menu) | Mover para o grupo ▸ (cada grupo, "Novo grupo" e "Fora dos grupos") |
| Painel de estilo | Título, Cor, Contorno e Apagar, quando um grupo está selecionado |
| Seleção | clique no título seleciona; texto ↔ desenho ligados como no nó |
| Desenho | `style G fill/stroke/stroke-dasharray/color` vale na moldura; seta não passa sobre o título |

**Should:** nada além do Must.

**Could:** nada.

**Won't (fica para depois):**

- Arrastar um nó para dentro ou para fora da moldura para mudar o grupo.
- `direction` do grupo pelo menu (o layout ainda ignora `direction` quando uma seta cruza a moldura).
- Copiar, colar, duplicar e seleção múltipla.

## Reconhecimento do código

| Peça | Onde está | Uso |
|---|---|---|
| Grupos no modelo | `Subgraph { id, title, parent, direction, line }` (`model.rs`) | igual; falta o span do cabeçalho e do `end` |
| Pertença do nó | `Builder::mention` (`flowchart.rs`): o último `subgraph` aberto que cita o id fica com o nó; citação fora de grupo não tira | é a regra do Mermaid (exemplo `c1-->a2` da documentação, conferido no CLI); as edições seguem ela |
| Moldura e título | `flow_render.rs`, laço "Clusters, outermost first"; cores em `cluster_colors` | estilo próprio entra ali |
| Estilo de nó | `node_style` + `fc.node_styles` (o parser já guarda `style G ...` de grupo ali) | mesma leitura para a moldura |
| Editar estilo | `edit::set_node_style` (recusa id que não é nó), `own_node_style` | passa a aceitar id de grupo |
| Deslocamento do grupo | chave `sub:<id>` no `%% mermaidcraft:layout` (`group_offset_key`) | sai junto quando o grupo some |
| Arraste do grupo | `paint::group_handle_at` (faixa do título), `Hit::Cluster` | clique sem arraste na faixa passa a selecionar |
| Seleção | `enum Sel { None, Node(String), Edge(usize) }` (`app.rs`) | ganha `Group(String)` |
| Menu | `MenuTarget`, `MenuAction`, `menu_ui`, `run_menu` (0.7.0) | ganha `Group(String)` e as ações novas |
| Texto → desenho | `edit::item_at` → `TextItem::{Node, Edge}` | ganha `TextItem::Group` |
| Remoção | `rebuild` com `Removals { nodes, edges }` | apagar com o conteúdo usa a mesma remoção |

Sem duplicidade nem conflito. Nenhuma dependência nova.

## Regras

1. **Selecionar.** Um clique na faixa do título seleciona o grupo; arrastar a faixa continua
   movendo o grupo. Um clique dentro da moldura, fora de nó e seta, desfaz a seleção (como hoje).
   O grupo selecionado ganha o contorno de seleção e a linha `subgraph` dele fica realçada no texto;
   o cursor na linha `subgraph` seleciona o grupo.
2. **Botão direito** dentro da moldura, fora de nó e seta, seleciona o grupo mais interno sob o
   ponteiro e abre o menu dele. O alvo fica preso na abertura, como na 0.7.0.
3. **Renomear** muda só o título e mantém o id: `subgraph G1["Título"]` vira
   `subgraph G1["Novo"]`. Num cabeçalho sem id próprio (`subgraph Entrada`), o id antigo é escrito
   na frente (`subgraph Entrada["Novo"]`) para as setas e o deslocamento que usam o id continuarem
   valendo. F2 e Enter abrem o título, como no nó.
4. **Cor e Contorno** gravam `style G fill:...,stroke:...` e `stroke-dasharray`, com a mesma paleta
   e a opção "Automática" do nó. A moldura usa `fill`, `stroke`, `stroke-dasharray` e `color` (cor
   do título) quando existem; sem `style`, nada muda no desenho.
5. **Mover para o grupo ▸** no menu do nó lista os grupos (os de dentro recuados), "Novo grupo" e
   "Fora dos grupos". Mover o nó X:
   - as linhas soltas de X (`X` ou `X[Rótulo]`) dentro de outros grupos saem;
   - cada seta de X escrita dentro de outro grupo sai para uma linha própria fora dos grupos (como o
     `reverse_edge` faz numa corrente); os outros nós da linha ficam onde estavam;
   - a definição de X (`X[Rótulo]`, forma) vai para uma linha antes do `end` do grupo de destino, ou
     fora dos grupos em "Fora dos grupos";
   - rótulo, forma, estilo e setas de X continuam iguais.
   "Novo grupo" cria `subgraph <id livre>["Grupo"]` com X dentro, seleciona o grupo e abre o título.
6. **Novo nó aqui** no menu do grupo cria o nó dentro do grupo, no ponto clicado. No espaço vazio
   fora de grupos continua como na 0.7.0.
7. **Desagrupar** tira as linhas `subgraph` e `end` do grupo; o conteúdo passa para o grupo de
   fora (ou para fora dos grupos). Saem junto: o `direction` próprio do grupo, o `style G`, o
   `class ... G` e o deslocamento `sub:G`. As setas que ligam no grupo (`A --> G`) saem, porque
   sem o grupo o id `G` viraria um nó novo; a barra de estado diz quantas saíram.
8. **Apagar com o conteúdo** (menu, painel e tecla Delete com o grupo selecionado) apaga o grupo,
   os grupos de dentro, os nós deles e as setas desses nós, como o Delete de um nó. ⌘Z desfaz.
9. **Seta e título.** Nenhum trecho de seta cruza o retângulo do título de um grupo. A seta que
   entraria pela faixa do título entra ao lado dele.
10. **Fora do fluxograma** nada muda: o menu continua só com "Ajustar à janela".

## Critérios de aceite

```gherkin
Funcionalidade: grupos no canvas

  Contexto:
    Dado o fluxograma aberto
      """
      flowchart TD
        subgraph G1["Entrada"]
          A[Um] --> B[Dois]
        end
        C[Tres] --> G1
        D[Quatro]
      """

  Cenário: clique no título seleciona o grupo
    Quando clico no título "Entrada"
    Então o grupo G1 fica selecionado
    E a linha 2 do texto fica realçada

  Cenário: arrastar o título ainda move o grupo
    Quando arrasto o título "Entrada" 40 pontos para a direita
    Então o comentário de layout tem "sub:G1=40,0"
    E o grupo G1 fica selecionado

  Cenário: menu do grupo
    Quando clico com o botão direito dentro da moldura de G1, fora de A e B
    Então o grupo G1 fica selecionado
    E o menu mostra Renomear, Cor, Contorno, Novo nó aqui, Desagrupar e Apagar

  Cenário: renomear mantém o id
    Quando renomeio G1 para "Chegada"
    Então o texto tem 'subgraph G1["Chegada"]'
    E "C[Tres] --> G1" continua no texto

  Cenário: renomear um grupo sem id próprio
    Dado um grupo escrito "subgraph Entrada"
    Quando renomeio para "Chegada"
    Então o texto tem 'subgraph Entrada["Chegada"]'

  Cenário: cor do grupo
    Quando escolho uma cor da paleta no menu de G1
    Então o texto tem "style G1 fill:"
    E a moldura de G1 é desenhada com essa cor

  Cenário: pôr um nó no grupo
    Quando escolho Mover para o grupo ▸ Entrada no menu de D
    Então D é desenhado dentro da moldura de G1
    E o rótulo de D continua "Quatro"

  Cenário: tirar um nó de uma corrente do grupo
    Quando escolho Mover para o grupo ▸ Fora dos grupos no menu de B
    Então B é desenhado fora da moldura de G1
    E A continua dentro de G1
    E a seta de A para B continua no texto

  Cenário: novo grupo a partir de um nó
    Quando escolho Mover para o grupo ▸ Novo grupo no menu de D
    Então o texto tem um subgraph novo com D dentro
    E o título do grupo novo fica aberto para digitar

  Cenário: novo nó dentro do grupo
    Quando escolho Novo nó aqui no menu de G1
    Então o nó novo é desenhado dentro da moldura de G1, no ponto clicado

  Cenário: desagrupar
    Quando escolho Desagrupar no menu de G1
    Então o texto não tem "subgraph G1" nem o "end" dele
    E A e B continuam no texto, fora de grupos
    E a seta "C[Tres] --> G1" sai
    E a barra de estado diz que 1 seta saiu

  Cenário: apagar com o conteúdo
    Dado G1 selecionado
    Quando aperto Delete
    Então o texto não tem G1, A nem B
    E C e D continuam
    Quando aperto ⌘Z
    Então o texto volta ao de antes

  Cenário: o que não pode acontecer
    Então renomear não muda o id nem perde o deslocamento "sub:G1"
    E mover um nó não perde o rótulo, a forma, o estilo nem as setas dele
    E desagrupar não deixa seta apontando para um id que virou nó
    E nenhuma edição de grupo deixa o texto com erro de sintaxe
    E com o texto com erro, as ações do menu não mudam nada e a barra de estado diz por quê
    E no diagrama de sequência ou ER o menu continua só com "Ajustar à janela"

  Cenário: seta não passa sobre o título
    Dado o fixture "fluxo-longo.mmd"
    Quando ele é desenhado
    Então nenhum trecho de seta cruza o retângulo do título de nenhum grupo
```

## Plano

Duas fases, uma entrega (0.8.0). Nenhum passo do mantenedor antes do código. Cada fase fecha com
testes, notas e o "pode commitar". Implementação com Sonnet, ler `src/app.rs` por trechos (ver a
memória de economia de créditos).

### Fase A — motor

- [x] A.1 Modelo: `Subgraph` ganha `header: Span` (a linha `subgraph ...`) e `end: Option<Span>`
      (a linha `end`), preenchidos no parser. Teste de span num caso aninhado.
- [x] A.2 `edit::rename_group(src, id, title)` (regra 3, os dois formatos de cabeçalho, aspas no
      título). Testes.
- [x] A.3 `set_node_style` e `own_node_style` aceitam id de grupo (regra 4). Desenho: a moldura usa
      `fill`, `stroke`, `stroke-dasharray` e `color` do `style` do grupo, por cima de
      `cluster_colors`. Teste no motor (cor do retângulo com `Tag::Cluster`).
- [x] A.4 `edit::move_to_group(src, node, Option<group>) -> String` e
      `edit::new_group(src, node, title) -> (String, String)` (regra 5). Testes: nó solto para
      dentro; nó de corrente para fora (os outros ficam); de um grupo para outro; aninhado; o
      rótulo e a forma ficam; o parser confere o `parent` de cada nó depois.
- [x] A.5 `edit::ungroup(src, id) -> (String, usize)` (regra 7; devolve quantas setas saíram) e
      `edit::remove_group(src, id)` (regra 8, com os grupos e nós de dentro). Testes com grupo
      aninhado, `direction`, `style`, deslocamento `sub:G`, seta para o grupo.
- [x] A.6 `edit::item_at` devolve `TextItem::Group(id)` na linha `subgraph`. Teste.
- [x] A.7 Seta e título (regra 9) em `flow_render.rs`: o retângulo do título entra como obstáculo
      do trecho que chega à moldura; a seta entra ao lado. Teste: nenhum `edge_paths` cruza o
      título no `fluxo-longo.mmd` e num caso mínimo (nó fora acima, alvo no canto esquerdo do
      grupo). Os testes de layout que já existem passam.
- [x] A.8 `cargo test`, clippy nos três alvos, uma revisão do diff da fase (revisor com
      `model: "sonnet"`), notas, diff e commit sugerido.

### Fase B — app

- [x] B.1 `Sel::Group(String)`: validação depois de cada mudança do texto, realce do contorno,
      linha realçada no texto, cursor na linha `subgraph` seleciona (regra 1).
- [x] B.2 Clique sem arraste na faixa do título seleciona; o arraste da 0.4.0 segue igual.
- [x] B.3 Menu: `MenuTarget::Group(String)` (grupo mais interno sob o ponteiro, regra 2),
      `MenuAction::{Ungroup, MoveTo(Option<String>), NewGroup}`; itens do grupo e "Mover para o
      grupo ▸" no menu do nó; "Novo nó aqui" no grupo chama `move_to_group` depois do
      `add_node_at` (regra 6).
- [x] B.4 Painel de estilo do grupo: Título, Cor, Contorno, Apagar (reaproveita `swatch` e os
      métodos `set_node_colour`/`set_node_dashed`, que passam a receber o id de grupo também).
- [x] B.5 Teclado: F2 e Enter abrem o título; Delete apaga com o conteúdo.
- [x] B.6 Textos em português no `i18n.rs` (Mover para o grupo, Novo grupo, Fora dos grupos,
      Desagrupar, Apagar com o conteúdo, Título, e a mensagem de setas que saíram).
- [x] B.7 Testes sem janela no `mod gestures` para os cenários do aceite; o teste da regra 10 da
      0.7.0 passa a esperar o nó dentro do grupo.
- [x] B.8 README (seção de edição visual) e captura com o menu do grupo aberto
      (`MERMAIDCRAFT_SHOT_MENU` passa a aceitar id de grupo).
- [x] B.9 `cargo test`, clippy nos três alvos, uma revisão da branch inteira e uma do delta da
      correção, notas, diff e commit sugerido.

### Riscos

- **Seta e título (A.7)** é a parte de layout e pode mudar o traçado de outros diagramas. Se a
  troca mexer em mais do que as setas que cruzam um título, a alternativa é empurrar só o ponto de
  entrada na moldura para a direita do título, sem tocar no resto da rota.
- **Mover nó de corrente.** Tirar a seta de X para uma linha própria muda a ordem do texto (como o
  `reverse_edge` da 0.7.0). Nada se perde; a ordem é a mesma regra já aceita.
- **Apagar com o conteúdo pela tecla Delete** apaga bastante coisa de uma vez; o ⌘Z desfaz num
  passo só.

### Fora desta branch

- Os itens Won't do escopo.
- Pendências antigas do layout: auto-laço sempre pela direita, `direction` em `subgraph` com seta
  cruzando a moldura.

## Desvios do plano

Fase A:

- **Textos com erro de sintaxe.** As edições de grupo usam `load_checked`, que recusa o texto com
  erro (as edições de nó antigas seguem com `load`, sem checar). A mensagem diz a linha do erro.
- **`class` antes da definição.** O parser só dá classe a um nó que já apareceu. Por isso as linhas
  novas fora dos grupos (definição do nó e setas que saíram) entram antes da primeira linha
  `class`, e só na falta dela no fim do texto.
- **Linha solta redundante.** Mover um nó para dentro de um grupo apaga a linha solta que só tem esse
  nó (`D` ou `D[Rótulo]`), para não sobrar duplicata.
- **Recuo.** Desagrupar e apagar grupo recuam uma vez para a esquerda o que estava dentro, quando o
  grupo ocupava linhas próprias.
- **Contorno do grupo.** O padrão da moldura é tracejado (menos no visual clássico), então "sólido"
  precisa ser escrito: `stroke-dasharray:0` (ou `none`) desenha sólido. O app (fase B) grava `0` em
  "Sólido" e `5 5` em "Tracejado" para grupos.
- **`item_at` e id de grupo.** O teste antigo dizia que um id de grupo não é nó e devolvia nada; agora
  `item_at` devolve `TextItem::Group` (na linha `subgraph`, no id numa seta e na linha `style G`).
  O teste foi atualizado de propósito.
- **App.** `TextItem::Group` ganhou um braço provisório em `src/app.rs` (não seleciona nada) até a
  fase B criar `Sel::Group`.
- **Limite conhecido.** Num nó citado em mais de um grupo, a definição (`X[Rótulo]`) que estava só
  na citação de um grupo apagado com "Apagar grupo" pode se perder; o nó continua no outro grupo.

Fase B:

- **`MoveTo` por índice.** `MenuAction` é `Copy`, então `MoveTo(Option<usize>)` guarda o número do
  grupo em `flow.subgraphs`, não o id. É seguro porque o menu fecha quando o texto muda.
- **Cor do grupo.** A paleta grava a tonalidade `pale` no `fill` do grupo (a do nó é `light`).
- **Contorno.** Sem `style`, o grupo aparece "Tracejado" fora do visual clássico. Sólido grava
  `stroke-dasharray:0`; `0` e `none` só valem como sólido para grupo (num nó escrito à mão, `0`
  continua sendo traço).
- **Título vazio.** A caixa do título vazia não muda nada e a barra de estado diz "O grupo precisa
  de um título".
- **Duplo clique no título** abre o título; clicar na faixa do título também seleciona (arrastar
  seleciona o grupo movido).
- **Novo nó aqui no grupo** é um passo de desfazer só (nó criado, posto no grupo e deslocado).
- **Captura.** `MERMAIDCRAFT_SHOT_MENU` aceita também id de grupo.

## Testes

Fase A: 60 testes no motor (19 novos para grupos: spans, renomear, estilo na moldura, mover, novo
grupo, desagrupar, apagar, CRLF, texto limpo, classes, `item_at`, e dois de layout: nenhuma seta
cruza o título no `fluxo-longo.mmd` e nos casos mínimos, e o desvio não entra em nó vizinho).
`cargo test` e `cargo clippy --workspace --all-targets -- -D warnings` limpos no macOS e com
`--target` Linux e Windows. Revisão independente (uma passada): 2 achados altos (`class` dentro de
grupo e classe perdida ao mover), 2 médios (desvio do título entrando em nó; `:::classe` perdida),
2 baixos corrigidos (nó em dois grupos) e 1 baixo mantido (linhas `B` soltas sobrando ao mover um
nó de várias setas e ao desagrupar `A --> G1`: só sujeira, nada se perde).
Não conferido: os testes novos não passaram por verificação de mutação (código revertido para ver
o teste falhar), salvo o do título, que falhava antes do desvio; o desenho do `fluxo-longo.mmd`
foi visto uma vez a olho.

Fase B: 42 testes no app (10 novos em `mod gestures`: título seleciona e arrasta, menu do grupo,
renomear, cor e contorno, mover/novo grupo, novo nó dentro, desagrupar com a mensagem, apagar e
⌘Z, texto com erro, título vazio). Revisão independente (uma passada): 3 achados baixos corrigidos
(prefixo `sub:` tirado em dobro, `0` como sólido também em nó, recusa calada do título vazio) e 2
observações sem defeito (tags `<b>` no título perdidas ao salvar, como no rótulo de nó; `<br/>`
vira quebra de linha na caixa). Clippy limpo nos três alvos. Captura do menu em
`docs/img/menu-grupo.png`. Não conferido: clique de verdade com o mouse na janela (só testes sem
janela e a captura); Linux e Windows rodando; sem verificação de mutação nos testes novos.
