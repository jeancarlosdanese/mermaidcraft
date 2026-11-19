//! Interface texts in English and Brazilian Portuguese. English is the key.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Pt,
}

/// Picks the language from the environment or, on macOS, from the system settings.
pub fn detect() -> Lang {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = std::env::var(var) {
            if v.to_ascii_lowercase().starts_with("pt") {
                return Lang::Pt;
            }
            if !v.is_empty() && v != "C" && v != "POSIX" {
                return Lang::En;
            }
        }
    }
    if cfg!(target_os = "macos") {
        if let Ok(out) = std::process::Command::new("defaults").args(["read", "-g", "AppleLocale"]).output() {
            if String::from_utf8_lossy(&out.stdout).to_ascii_lowercase().starts_with("pt") {
                return Lang::Pt;
            }
        }
    }
    Lang::En
}

const PT: &[(&str, &str)] = &[
    ("File", "Arquivo"),
    ("Edit", "Editar"),
    ("View", "Exibir"),
    ("Help", "Ajuda"),
    ("New", "Novo"),
    ("New flowchart", "Novo fluxograma"),
    ("New sequence diagram", "Novo diagrama de sequência"),
    ("New ER diagram", "Novo diagrama ER"),
    ("Open…", "Abrir…"),
    ("Save", "Salvar"),
    ("Save As…", "Salvar como…"),
    ("Export", "Exportar"),
    ("Export SVG…", "Exportar SVG…"),
    ("Export PNG 1x…", "Exportar PNG 1x…"),
    ("Export PNG 2x…", "Exportar PNG 2x…"),
    ("Export PNG 3x…", "Exportar PNG 3x…"),
    ("Export PDF…", "Exportar PDF…"),
    ("Quit", "Sair"),
    ("Undo", "Desfazer"),
    ("Redo", "Refazer"),
    ("Zoom in", "Aumentar zoom"),
    ("Zoom out", "Diminuir zoom"),
    ("Fit to window", "Ajustar à janela"),
    ("Actual size", "Tamanho real"),
    ("Dark theme", "Tema escuro"),
    ("Language", "Idioma"),
    ("English", "Inglês"),
    ("Portuguese (Brazil)", "Português (Brasil)"),
    ("About MermaidCraft", "Sobre o MermaidCraft"),
    ("Supported diagrams: flowchart, sequence, ER.", "Diagramas suportados: fluxograma, sequência e ER."),
    ("Untitled", "Sem título"),
    ("Select", "Selecionar"),
    ("Link", "Ligar"),
    ("+ Node", "+ Nó"),
    ("Direction", "Direção"),
    ("Top to bottom", "De cima para baixo"),
    ("Bottom to top", "De baixo para cima"),
    ("Left to right", "Da esquerda para a direita"),
    ("Right to left", "Da direita para a esquerda"),
    ("Node", "Nó"),
    ("Edge", "Aresta"),
    ("Label", "Rótulo"),
    ("Shape", "Forma"),
    ("Line", "Linha"),
    ("Arrow", "Seta"),
    ("Delete", "Apagar"),
    ("New node", "Novo nó"),
    ("Drag nodes to rearrange them. Click to edit. + Node adds a node; Link (or Shift+drag from one node to another) connects them. The scroll wheel zooms.", "Arraste os nós para reorganizar. Clique para editar. + Nó cria um nó; Ligar (ou Shift+arrastar de um nó a outro) conecta. A roda do mouse dá zoom."),
    ("Visual editing works on flowcharts; edit the text for this diagram type.", "A edição visual funciona em fluxogramas; edite o texto para este tipo de diagrama."),
    ("Click the node to connect to", "Clique no nó de destino"),
    ("Click a node or an edge to edit it. + Node adds a node; Link (or dragging from one node to another) connects them. Drag a group by its title to move it.", "Clique num nó ou numa seta para editar. + Nó cria um nó; Ligar (ou arrastar de um nó a outro) conecta. Arraste um grupo pelo título para movê-lo."),
    ("Moved a group", "Grupo movido"),
    ("Problems", "Problemas"),
    ("No problems", "Nenhum problema"),
    ("Line", "Linha"),
    ("error", "erro"),
    ("errors", "erros"),
    ("nodes", "nós"),
    ("edges", "arestas"),
    ("Nothing to draw yet.", "Ainda não há o que desenhar."),
    ("Fix the errors before exporting.", "Corrija os erros antes de exportar."),
    ("Exported to", "Exportado para"),
    ("Saved", "Salvo"),
    ("Could not save", "Não foi possível salvar"),
    ("Could not open", "Não foi possível abrir"),
    ("Could not export", "Não foi possível exportar"),
    ("Mermaid blocks", "Blocos Mermaid"),
    ("Block", "Bloco"),
    ("No mermaid blocks in this file.", "Este arquivo não tem blocos mermaid."),
    ("Add block", "Adicionar bloco"),
    ("Unsaved changes", "Alterações não salvas"),
    ("Save the changes before closing?", "Salvar as alterações antes de fechar?"),
    ("Discard", "Descartar"),
    ("Cancel", "Cancelar"),
    ("The file is too large.", "O arquivo é grande demais."),
    ("Edit the Mermaid text here", "Edite o texto Mermaid aqui"),
    ("Created a node", "Nó criado"),
    ("Created a link", "Ligação criada"),
    ("Created a node and a link", "Nó e ligação criados"),
    ("solid", "contínua"),
    ("dotted", "pontilhada"),
    ("thick", "grossa"),
    ("invisible", "invisível"),
    ("none", "nenhuma"),
    ("arrow", "seta"),
    ("circle", "círculo"),
    ("cross", "cruz"),
    ("rect", "retângulo"),
    ("round", "arredondado"),
    ("stadium", "estádio"),
    ("subroutine", "sub-rotina"),
    ("cylinder", "cilindro"),
    ("double-circle", "círculo duplo"),
    ("diamond", "losango"),
    ("hexagon", "hexágono"),
    ("parallelogram", "paralelogramo"),
    ("parallelogram-alt", "paralelogramo (inverso)"),
    ("trapezoid", "trapézio"),
    ("trapezoid-alt", "trapézio (inverso)"),
    ("asymmetric", "assimétrico"),
    ("Zoom", "Zoom"),
    ("Appearance", "Aparência"),
    ("Font", "Fonte"),
    ("Colors", "Cores"),
    ("By group", "Por grupo"),
    ("By shape", "Por forma"),
    ("Neutral", "Neutras"),
    ("One per node", "Uma por nó"),
    ("Classic (Mermaid)", "Clássicas (Mermaid)"),
    ("Fill", "Preenchimento"),
    ("Solid", "Sólido"),
    ("Hatched", "Hachura"),
    ("Cross-hatched", "Hachura cruzada"),
    ("Stroke", "Traço"),
    ("Clean", "Limpo"),
    ("Artist", "Artista"),
    ("Cartoonist", "Cartunista"),
    ("Arrows", "Setas"),
    ("Graphite", "Grafite"),
    ("Color of the source node", "Cor do nó de origem"),
    ("Restore default", "Restaurar padrão"),
    ("Auto-arrange", "Organizar automático"),
    ("Moved a node", "Nó movido"),
    ("Version", "Versão"),
    ("Drop a .mmd or .md file here to open it.", "Solte um arquivo .mmd ou .md aqui para abri-lo."),
];

/// Translates an English text.
pub fn t(lang: Lang, en: &'static str) -> &'static str {
    match lang {
        Lang::En => en,
        Lang::Pt => PT.iter().find(|(k, _)| *k == en).map_or(en, |(_, v)| v),
    }
}
