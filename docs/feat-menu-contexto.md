# FEAT: menu de botão direito no canvas

Estado: entregue na 0.7.0 (20/11/2026)

## Problema

As ações de um nó ou de uma seta ficam espalhadas: forma, cor e contorno no painel flutuante,
ligar na barra (ferramenta "Ligar") ou no arraste com Shift, apagar no painel ou na tecla Delete,
renomear com dois cliques. Quem não conhece esses caminhos acha que o canvas só sabe criar nós
(relato do Jean na 0.6.1: "A única coisa que consegui fazer foi add um nó"). O lugar que todo
editor gráfico usa, o botão direito sobre o próprio componente, não faz nada.

## Objetivo

O botão direito sobre um nó, uma seta ou o espaço vazio abre um menu com o que dá para fazer
ali. Cada ação do menu usa a mesma edição do texto que o painel e a barra já usam: o desenho e o
texto continuam iguais, venha a mudança de onde vier.

KRs:

1. Todas as ações do painel de estilo e da barra que valem para um componente estão no menu dele.
2. Nenhuma ação nova de edição do texto sem teste no motor; nenhum gesto do menu sem teste sem
   janela (`mod gestures`).
3. Os gestos que já existem (clique, dois cliques, arraste, Shift+arraste, roda) continuam iguais:
   os testes da 0.6.2 passam sem mudança.

## Escopo

**Must** (aprovado pelo Jean em 20/11/2026):

| Onde | Itens |
|---|---|
| Nó | Renomear, Forma ▸, Cor ▸, Contorno ▸, Ligar a…, Novo nó ligado, Apagar |
| Seta | Rótulo, Linha ▸, Ponta ▸, Inverter sentido, Apagar |
| Espaço vazio | Novo nó aqui, Organizar automático, Ajustar à janela, Direção ▸ |
| Teclado | Delete (ou Backspace) apaga; Enter ou F2 renomeia o nó ou edita o rótulo da seta |

O painel de estilo continua como está.

**Should:** o atalho aparece ao lado do item no menu (Delete, F2), como no menu Editar.

**Could:** nada.

**Won't (fica para depois):**

- Menu próprio de grupo (`subgraph`): renomear, mover para dentro, apagar o grupo.
- Copiar, recortar, colar e duplicar.
- Seleção de vários componentes ao mesmo tempo.
- Menu nos diagramas de sequência e ER (lá a edição visual não existe).

## Reconhecimento do código

| Ação | Já existe em | Reaproveitamento |
|---|---|---|
| Renomear nó | `focus_label` + caixa "Rótulo" do painel (`inspector`) | o mesmo `focus_label` |
| Rótulo da seta | caixa "Rótulo" do painel, sem foco automático | `focus_label` passa a valer para seta também |
| Forma | `edit::set_node(.., Some(shape))`, `NodeShape::all()` | igual |
| Cor | `restyle` + `engine::look::SWATCHES`, opção "Automática" | igual; o laço vira uma função usada pelo painel e pelo menu |
| Contorno | `restyle` com `stroke-dasharray` | igual |
| Ligar a… | `link_from` + ferramenta Ligar (`Tool::Link`) | `link_from` passa a valer também com a ferramenta Selecionar |
| Novo nó ligado | fim do Shift+arraste no vazio (`add_node` + `add_edge`) | o trecho vira um método usado pelos dois |
| Apagar | `delete_selected` (painel e tecla Delete) | igual |
| Linha, Ponta | `edit::set_edge(.., Some(kind))` | igual |
| Inverter sentido | **não existe** | `edit::reverse_edge`, novo no motor |
| Novo nó aqui | `add_node` (sempre no fim do layout) + `set_offsets` | nó novo e deslocamento até o ponto clicado |
| Organizar automático | `auto_arrange` | igual |
| Ajustar à janela | `auto_fit = true` | igual |
| Direção | `edit::set_direction` (caixa da barra) | igual |
| Menu do egui | `Response::context_menu` e `secondary_clicked` (egui 0.36.2) | sem dependência nova |

Sem duplicidade nem conflito. Hoje o canvas é uma resposta só (`allocate_exact_size`), e o
botão direito não faz nada nele.

## Regras

1. **O botão direito seleciona.** Sobre um nó ou uma seta, ele seleciona o componente, como o
   clique, e o painel de estilo mostra o mesmo componente que o menu. Sobre o vazio, desfaz a
   seleção.
2. **O alvo fica preso na abertura.** O menu age sobre o que estava sob o ponteiro quando abriu,
   mesmo que o ponteiro passe por cima de outro componente depois.
3. **A vista não desliza com o menu aberto.** O deslize da 0.6.1, que tira a seleção de baixo do
   painel, espera o menu fechar. Sem isso o nó sairia de baixo do menu.
4. **Ligar a…** marca o nó em laranja, como a ferramenta Ligar, sem trocar de ferramenta. O
   próximo clique num outro nó cria a seta; clique no vazio ou Esc desiste.
5. **Novo nó ligado** cria o nó "Novo nó" e a seta do nó atual até ele, seleciona o novo e abre o
   rótulo para digitar, como o Shift+arraste solto no vazio. O layout decide o lugar.
6. **Novo nó aqui** cria o nó e o deixa no ponto clicado. O deslocamento fica no comentário
   `%% mermaidcraft:layout`, como o de um nó arrastado; "Organizar automático" o apaga.
7. **Inverter sentido** troca os nós das pontas e mantém o traço como está: `A --> B` vira
   `B --> A`, e `A x--> B` vira `B x--> A` (a cruz continua na ponta de saída, agora em B). O
   rótulo, a linha e o comprimento ficam. Trocar as cabeças no traço não serve: o Mermaid não
   aceita cabeça só no começo (`B o-- A`, `B <-- A` são erro). Numa ligação sozinha na linha (`A --> B`, com ou sem a definição dos nós,
   `A[Um] --> B[Dois]`), a troca é feita no lugar. Numa corrente (`A --> B --> C`) ou numa
   ligação compartilhada (`A & B --> C`), a seta sai para uma linha própria no fim, como já faz a
   troca de rótulo dessas setas. A seleção acompanha a seta.
8. **Enter e F2** abrem o rótulo do selecionado. Não valem enquanto o teclado está numa caixa de
   texto (o editor de texto ou o próprio rótulo).
9. **Fora do fluxograma** (sequência, ER) o menu tem só "Ajustar à janela".
10. **Sobre um grupo** vale o menu do espaço vazio. "Novo nó aqui" põe o nó no ponto clicado, mas
    ele não entra no grupo (o texto o declara fora do `subgraph`).

## Critérios de aceite

```gherkin
Funcionalidade: menu de botão direito no canvas

  Contexto:
    Dado o fluxograma "flowchart TD\n  A[Um] --> B[Dois]\n  C[Tres]\n" aberto

  Cenário: o menu de um nó age sobre ele
    Quando clico com o botão direito em A
    Então A fica selecionado
    E o menu mostra Renomear, Forma, Cor, Contorno, Ligar a…, Novo nó ligado e Apagar

  Cenário: forma pelo menu
    Quando abro o menu de A e escolho Forma ▸ Losango
    Então o texto tem "A{Um}"

  Cenário: cor pelo menu
    Quando abro o menu de C e escolho uma cor da paleta
    Então o texto ganha "style C fill:…,stroke:…"
    E "Automática" no mesmo menu apaga essa linha

  Cenário: ligar a outro nó
    Quando abro o menu de A, escolho "Ligar a…" e clico em C
    Então o texto ganha "A --> C"

  Cenário: desistir de ligar
    Quando abro o menu de A, escolho "Ligar a…" e tecle Esc
    E clico em C
    Então o texto não muda

  Cenário: novo nó ligado
    Quando abro o menu de B e escolho "Novo nó ligado" e digito "Fim" e tecle Enter
    Então o texto tem um nó novo "Fim" e a seta "B --> " até ele

  Cenário: apagar pelo menu e pelo teclado
    Quando abro o menu de C e escolho Apagar
    Então C some do texto
    E com B selecionado, a tecla Delete apaga B e a seta A --> B

  Cenário: inverter uma seta sozinha na linha
    Quando abro o menu da seta A --> B e escolho "Inverter sentido"
    Então a linha passa a ser "B[Dois] --> A[Um]"
    E a seta continua selecionada

  Cenário: inverter numa corrente
    Dado o texto "flowchart TD\n  A --> B --> C\n"
    Quando inverto a seta B --> C
    Então o texto tem "A --> B" e uma linha "C --> B"
    E a seta A --> B não muda

  Cenário: inverter mantém rótulo, linha e cabeças
    Dado o texto "flowchart TD\n  A x-.->|sim| B\n  C ---o D\n"
    Quando inverto as duas setas
    Então o texto tem "B x-.->|sim| A" e "D ---o C", e continua sem erro

  Cenário: novo nó no ponto clicado
    Quando clico com o botão direito no vazio e escolho "Novo nó aqui"
    Então nasce um nó com o centro no ponto clicado (com folga de 2 px)
    E o deslocamento dele fica no comentário "%% mermaidcraft:layout"

  Cenário: direção pelo menu
    Quando abro o menu do vazio e escolho Direção ▸ Esquerda para a direita
    Então o cabeçalho passa a ser "flowchart LR"

  Cenário: F2 e Enter abrem o rótulo
    Com A selecionado, F2 põe o teclado no rótulo de A com o texto marcado
    E com a seta A --> B selecionada, Enter põe o teclado no rótulo da seta

  # O que não pode acontecer
  Cenário: o alvo não muda com o ponteiro
    Quando abro o menu de A, passo o ponteiro sobre C e escolho Apagar
    Então A some e C continua

  Cenário: atalho não age dentro de uma caixa de texto
    Com o cursor no editor de texto, Delete, Enter e F2 não mexem no desenho

  Cenário: a vista não desliza com o menu aberto
    Dado um nó sob o painel de estilo
    Quando abro o menu dele
    Então a vista não se move até o menu fechar

  Cenário: os gestos de antes continuam
    Os testes de gestos da 0.6.2 passam sem mudança

  Cenário: diagrama que não é fluxograma
    Dado um diagrama de sequência
    Quando clico com o botão direito no canvas
    Então o menu tem só "Ajustar à janela" e o texto não muda
```

## Plano

Fase única, numa entrega (0.7.0). Nenhum passo do mantenedor antes do código.

- [x] 1. Motor: `edit::reverse_edge(src, edge)` com os casos da regra 7 (sozinha na linha, com
      definição dos nós, corrente, compartilhada, cabeças, rótulo; o texto continua sem erro) e testes em
      `crates/engine/tests/behaviour.rs`.
- [x] 2. App, preparação sem mudar comportamento: tirar do painel o laço da paleta de cores e o
      "Novo nó ligado" do Shift+arraste para métodos que o menu também chama. Os testes de gestos
      seguem passando.
- [x] 3. App: estado do menu (`menu_target`, preso na abertura), seleção no botão direito,
      `context_menu` com os três menus e o de fora do fluxograma; "Ligar a…" com a ferramenta
      Selecionar; "Novo nó aqui" com o deslocamento; `focus_label` também para seta; o deslize
      espera o menu fechar.
- [x] 4. Teclado: Enter e F2.
- [x] 5. Textos em português no `i18n.rs` (Renomear, Ligar a…, Novo nó ligado, Inverter sentido,
      Novo nó aqui e os submenus).
- [x] 6. Testes sem janela no `mod gestures` para os cenários acima (o botão direito entra como
      `PointerButton::Secondary`; o item do menu é clicado pela posição que o egui deu a ele).
- [x] 7. README (seção de edição visual) e captura do menu aberto em `docs/img/`.
- [x] 8. `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, e o `cargo clippy`
      para Linux e Windows (`--target`); revisão independente do diff; diff e mensagem de commit
      para o "pode commitar".

### Riscos

- **Clique no item do menu no teste sem janela.** O egui só sabe a posição dos itens depois de
  desenhar o menu. Se não der para achá-los com confiança, os testes chamam as ações pelo mesmo
  método que o item chama, e o clique de verdade fica para o teste manual.
- **Novo nó aqui mexe nos outros.** Um nó novo entra no layout automático e pode deslocar os
  vizinhos que não foram arrastados (já acontece com "+ Nó"). O nó novo fica no ponto clicado; os
  outros, onde o layout mandar.
- **Captura do menu.** O modo `--screenshot` não abre menu. Se não houver como, a captura do README
  é feita à mão pelo Jean ou fica de fora.

### Fora desta branch

- Menu de grupo, copiar e colar, seleção múltipla (Won't).
- Pendências antigas do layout (seta sobre o título do grupo, auto-laço, `direction` em
  `subgraph`).

## Desvios do plano

- **Inverter numa corrente põe a linha nova antes do comentário de layout** (`insert_line`), não
  no fim do arquivo como faz a troca de rótulo (`set_edge`, que usa `append_line`). O índice da
  seta devolvido procura a seta só a partir dessa linha, para não pegar uma seta igual escrita
  depois do comentário (achado da revisão do passo 1).
- **CRLF ao desfazer uma corrente**: o `rebuild` do motor juntava as partes com `\n` mesmo em
  arquivo CRLF. Agora usa a quebra do arquivo. Vale também para apagar nó ou seta (achado da
  revisão do passo 1, erro antigo na área da FEAT).
- **Seleção da seta ao trocar linha ou ponta**: numa ligação compartilhada (`A & B --> C`) a seta
  vai para uma linha própria e muda de número. O painel deixava a seleção no número antigo (outra
  seta); agora o painel e o menu usam `set_edge_kind`, que segue a seta.
- **Atalho de apagar no menu aparece como "Del"**: a fonte não tem o símbolo ⌫.
- **Captura com o menu aberto**: `MERMAIDCRAFT_SHOT_MENU=<id>` com `--screenshot` abre o menu do
  nó sem clique (lido só nesse modo). Captura em `docs/img/menu-contexto.png`.
- **O menu fecha quando o texto muda por baixo dele** (Delete, desfazer): o alvo é um id de nó ou
  um número de seta daquele texto, e uma seta renumerada levaria a ação para outra (achado médio da
  revisão do app). F2 e Enter não agem com o menu aberto.
- **"Ligar a…" e a ferramenta Ligar guardam o id do nó de origem**, não o número: apagar o nó de
  origem desiste da ligação em vez de passá-la a outro nó (achado médio da revisão; o caminho já
  existia na ferramenta Ligar, o menu o tornava comum).
- **Rótulo em digitação e botão direito**: a caixa do rótulo só perde o teclado quando o botão é
  solto. O botão direito vê o que está sob o ponteiro no desenho da tela, salva o rótulo e só então
  grava o alvo: o nó pelo id, e a seta procurada de novo pelas pontas, tipo e rótulo (o rótulo
  recém-salvo, se era o dela), porque salvar o rótulo pode renumerar as setas ou alargar um nó. Sem
  seta igual, o menu é o do espaço vazio (achados médios da segunda à quarta passada de revisão).
- **O submenu da seta se chama "Ponta"**, como na especificação; o painel continua com "Seta".
- **Itens do menu clicados de verdade nos testes**: o menu guarda onde desenhou cada item
  (`menu_rects`), e os testes clicam nessa posição, abrindo os submenus pelo ponteiro. O risco
  previsto (chamar a ação direto) não se confirmou.

## Testes

Conferido:

- Motor: `edit_reverse_edge_*` (5 testes): sozinha na linha com definição dos nós, corrente,
  compartilhada, cabeças e rótulo, comentário de layout, laço, erros, seta igual depois do
  comentário, CRLF.
- App, sem janela (`mod gestures`, 24 testes novos, clique de verdade nos itens): menu do nó e
  seleção, Renomear e Rótulo pelo menu, Linha e Ponta numa ligação compartilhada (a seleção segue
  a seta), Organizar automático, desistir de ligar com clique no vazio, menu que fecha quando
  Delete muda o texto, ligação pendente de um nó apagado, menu que abre com um rótulo em digitação (curto, e longo que alarga o nó), seta
  compartilhada achada de novo depois do rótulo salvo, seta paralela com outro rótulo, seta achada
  pelo rótulo novo, forma, cor (paleta e Automática), contorno, Ligar a… e desistir com Esc, Novo nó ligado
  com rótulo digitado, apagar pelo menu e pela tecla, inverter (sozinha e em corrente, seleção
  segue a seta), Novo nó aqui (centro a até 2 px do ponto), direção, F2 e Enter (nó e seta, texto
  marcado), alvo preso com o ponteiro sobre outro nó, atalhos ignorados no editor de texto, vista
  parada com o menu aberto e deslizando depois que ele fecha, diagrama de sequência só com
  "Ajustar à janela".
- O teste da vista parada, o do menu que fecha, o do rótulo em digitação e os dois da busca da seta
  falham se a correção for tirada (mutação conferida).
- Os 8 testes de gestos da 0.6.2 passam sem mudança. Total: 83 testes verdes.
- `cargo clippy --workspace --all-targets -D warnings` limpo no macOS e com `--target` Linux e
  Windows.
- Captura do app com o menu aberto (modo `--screenshot`, português).

Não conferido:

- Clique de verdade com o mouse na janela (só o teste sem janela e a captura): fica para o Jean.
- Linux e Windows rodando (só compilação e clippy).
- `cargo fmt`: o projeto não segue o rustfmt desde antes desta branch; nada foi reformatado.

