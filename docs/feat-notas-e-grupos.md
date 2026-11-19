# FEAT: notas ao lado dos nós e grupos que se movem inteiros

Estado: implementado, aguardando revisão · 18/11/2026

## Objetivo

1. Num fluxograma com notas de achado ligadas a etapas (`X01["F01 …"] -.- B`), cada nota fica ao
   lado do nó que comenta, dentro do grupo dele. Hoje as notas ficam soltas fora dos grupos, numa
   fileira, e os grupos descem em escada.
2. No app, mover um grupo inteiro (moldura e tudo o que está dentro) arrastando, como já se faz
   com um nó.

## Regras

**O que é uma nota.** Um nó solto (fora de qualquer grupo) com uma única ligação, sem seta em
nenhuma ponta (`---`, `-.-`, `===`) e sem rótulo, até um nó que está dentro de um grupo. A nota
passa a fazer parte do grupo desse nó e é posta ao lado dele: à direita num fluxo vertical
(TD/BT), abaixo num horizontal (LR/RL). Várias notas do mesmo nó ficam empilhadas ao longo do
fluxo, e acompanham o nó quando ele é arrastado. Um nó com laço em si mesmo não recebe notas (o
laço ocupa esse lado). Nenhum nó declarado num grupo sai dele; nada muda em diagramas sem notas.

**Como a nota ganha lugar.** O layout reserva espaço num lado só do nó (`LItem::side`), sem mudar
o centro dele, então as setas que entram e saem do nó continuam retas.

**Grupo movido.** O deslocamento fica no comentário `%% mermaidcraft:layout` com a chave
`sub:<id do grupo>`, para não colidir com o id de um nó. Vale para a moldura, os nós, as notas e
os grupos internos; o deslocamento de um nó dentro do grupo soma por cima. As setas internas do
grupo andam junto.

**No app.** O grupo é arrastado pela faixa do título (30 px no topo da moldura); o cursor vira uma
mão aberta ali. Arrastar o resto da moldura continua movendo a tela. "Organizar automático" apaga
também os deslocamentos de grupo. A cena guarda as molduras (`Scene::cluster_boxes`), então isso
funciona em qualquer traço. Um grupo com id repetido, ou com espaço, `=` ou `,` no id, não se
arrasta (a posição não teria como ser guardada só para ele).

## Critérios de aceite

1. Nota ao lado do seu nó, na mesma altura, dentro da moldura do grupo; o fluxo principal segue
   numa coluna reta (`notes_sit_beside_their_node_inside_its_group`).
2. `A --- B` sem grupos continua com B abaixo de A (`plain_headless_links_are_not_notes`).
3. `sub:G=120,40` desloca tudo o que está em G em exatamente (120, 40), e a chave sobrevive à
   escrita e à leitura do comentário (`moving_a_group_moves_everything_inside`).
4. Diagramas sem notas e sem grupo movido saem idênticos aos da 0.3.0 (conferido com os exemplos e
   os casos de teste da etapa anterior).
5. `cargo test` e `cargo clippy -- -D warnings` limpos.

## Revisão (18/11/2026)

Revisor independente, com casos de borda e comparação byte a byte com a 0.3.0. Corrigidos:

- Alta: uma folha declarada num grupo, ligada sem seta a algo de fora, virava nota e deixava o
  grupo vazio. Agora só nós soltos viram nota (teste em `plain_headless_links_are_not_notes`).
- Alta: com o traço à mão (o padrão), a moldura vira polígono e o arraste de grupo não achava o
  grupo. A cena passou a guardar as molduras (`group_frames_are_known_in_every_look`).
- Média: a nota não acompanhava o nó arrastado (`a_note_follows_its_node_when_moved`).
- Média: grupos com o mesmo id se moviam juntos; agora o arraste é recusado.
- Média: rótulo na ligação da nota, ou laço no nó, ficavam escondidos; esses casos não viram nota.
- Baixa: id de grupo com espaço ou `=` não guardava a posição; o arraste é recusado. Código morto
  removido.

Ficam para depois: ao mover um grupo interno, a moldura do pai é recalculada sem olhar a largura do
título (já acontecia ao mover nós); em LR com duas notas, a ligação da segunda faz uma curva junto
das setas de saída.

## Atenção

Deslocamentos guardados antes desta versão foram medidos no layout antigo. Num arquivo cujas notas
foram arrastadas à mão, eles passam a empurrar as notas para longe do lugar novo: "Organizar
automático" limpa o comentário e deixa o layout novo agir.
