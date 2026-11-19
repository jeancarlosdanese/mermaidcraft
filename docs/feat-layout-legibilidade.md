# FEAT: layout legível e preferências compartilhadas

Estado: entregue na 0.3.0 (18/11/2026)

## Objetivo

Fazer diagramas reais e longos ficarem legíveis sem ajuste manual. O caso de teste é um fluxograma
de produto com cinco grupos, losangos de texto longo e setas entre grupos (o `fluxo-do-turno.mmd`
do HyVox). E fazer o CLI e o MCP seguirem as preferências de visual escolhidas no app.

## Problemas observados

1. Rótulos longos não quebram linha: um losango com uma pergunta de 70 caracteres ocupa a largura
   do grupo inteiro e espalha o layout.
2. Setas entre grupos são desenhadas do centro de um nó ao centro do outro e atravessam nós no
   caminho.
3. Várias setas que chegam ao mesmo nó (ou saem dele) pelo mesmo lado usam o mesmo ponto, e as
   pontas ficam sobrepostas.
4. O rótulo de uma seta pode cair em cima da borda de um grupo ou de um nó.
5. A dica da barra de ferramentas não tem tradução para português.
6. O CLI e o MCP sempre usam o visual padrão; a escolha feita no app não vale para eles.

Pendências da 0.2.0 que entram junto:

7. Com "cor do nó de origem", a seta ignora o `style` e o `classDef` do nó.
8. Exportar em PNG um diagrama grande demais gasta gigabytes antes de recusar o tamanho.

## Solução

1. **Quebra automática** por palavras, medida com a fonte escolhida: até 220 px por linha nos nós
   comuns, 150 px nos losangos, 120 px nos círculos e 150 px nos rótulos de seta. Quebras
   explícitas (`<br>`) continuam valendo; uma palavra mais longa que a linha fica inteira.
2. **Desvio de nós:** depois de traçar uma seta, cada trecho que cruza a forma de um nó que não é
   ponta da seta ganha um desvio por um dos lados, com folga. O lado escolhido é o de caminho mais
   curto, e cada cruzamento com outra seta pesa como um desvio longo. Repete até não cruzar mais
   nada (no máximo 16 desvios por seta). Setas entre nós de grupos diferentes deixam de seguir os
   pontos do layout entre grupos (que levam em conta só as molduras) e são traçadas diretas,
   desviando também das molduras de grupos que não contêm nenhuma das pontas.
3. **Portas distribuídas:** setas que chegam a um nó pelo mesmo lado, ou saem dele, são espalhadas
   ao longo desse lado na ordem em que vêm, e entram retas na borda.
4. **Rótulo livre:** o rótulo procura, ao longo da própria seta, a posição mais perto do meio que
   não encosta em nó nem cruza a borda de grupo.
5. Tradução da dica.
6. **Preferências compartilhadas:** o app, o CLI e o MCP leem o mesmo arquivo
   (`~/Library/Application Support/MermaidCraft/settings.txt`; outro caminho pela variável
   `MERMAIDCRAFT_SETTINGS`). As opções `--look`, `--font` etc. valem por cima dele; `--look` define
   a base inteira (`sketch` = padrão, `clean` = visual clássico).
7. A cor da seta vem do contorno final do nó, já com `style` e `classDef`.
8. O tamanho do PNG é conferido pelas medidas do diagrama, antes de montar o SVG.

Detalhe da curva: as alças da curva de cada trecho ficam limitadas a um terço do trecho, para
trechos curtos (como o de entrada numa porta) não fazerem laço.

## Critérios de aceite

1. Num fluxograma com rótulos longos, nenhum nó passa da largura máxima mais as margens da forma.
2. No caso de teste, nenhuma seta cruza a caixa de um nó que não seja ponta dela.
3. Setas que chegam ao mesmo nó pelo mesmo lado terminam em pontos diferentes.
4. Com `stroke=clean` gravado nas preferências, `mermaidcraft-cli render` sai com traço limpo sem
   nenhuma opção na linha de comando.
5. `cargo test` e `cargo clippy -- -D warnings` limpos.

O caso de teste é `crates/engine/tests/fixtures/fluxo-longo.mmd`: mesma estrutura do fluxograma
real (cinco grupos, losangos longos, setas entre grupos, `classDef`), com rótulos neutros. Testes:
`long_labels_wrap`, `edges_do_not_cross_other_nodes`, `edges_into_one_side_get_their_own_ports`,
`oversized_png_is_refused_before_drawing` e, em `prefs.rs`, a leitura e a escrita do visual.

## Revisão (18/11/2026)

Revisor independente, com testes de mutação. Corrigidos:

- Alta: em seta reta (dois pontos), o rótulo caía na ponta, em cima do nó. Os candidatos agora são
  tirados por comprimento ao longo da seta (`point_along`). Teste: `straight_edge_label_sits_in_the_middle`.
- Média: seta invisível (`~~~`) entrava nas portas e no custo de cruzamento, entortando as visíveis.
  Teste: `invisible_edges_leave_ports_alone`.
- Média: dois testes passavam com a função desligada. O de portas agora confere pontas separadas
  e entrada reta; o de cruzamento confere pontos a cada 2 px, inclusive em trechos retos; o tamanho
  do PNG tem teste de unidade e passou a limitar também a área (128 milhões de pixels).
- Baixa: com "cor do nó de origem", contorno transparente deixava a seta invisível; agora usa a cor
  padrão da linha. Ajuda do CLI (`commands` e `convert`) atualizada.

## Fica para depois

- Uma seta pode passar por cima do título de um grupo (no caso de teste, a que chega em "Envio").
- O auto-laço sai sempre pelo lado direito e pode encostar em outra seta que saia por ali.
- `direction` dentro de `subgraph` continua ignorado quando alguma seta cruza a moldura.
- Pontos de desvio não são verificados de novo depois de `drop_reversals` e do trecho de porta
  (achado baixo da revisão, não reproduzido).
- O arquivo de preferências com UTF-8 inválido é ignorado inteiro, e o app grava sem troca atômica.
- `declared_size` lê o primeiro `<svg` do texto, mesmo dentro de um comentário de SVG externo.
- ~~Notas ligadas por `-.-` a nós dentro de grupos ficam numa fileira fora dos grupos, e os grupos
  descem em escada.~~ Resolvido na 0.4.0 (`feat-notas-e-grupos.md`).
