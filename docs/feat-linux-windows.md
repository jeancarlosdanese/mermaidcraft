# FEAT: Linux e Windows

Estado: entregue na 0.6.0 (19/11/2026)

## Objetivo

O MermaidCraft só era empacotado e testado no macOS. O Fedora instala pelo GitHub, então o suporte a
Linux e Windows precisa estar no repositório: compilar, abrir a janela, guardar as preferências no
lugar certo de cada sistema e ter um script de empacotamento por sistema.

## O que já funcionava

- Motor, CLI e MCP são Rust puro, sem nada do macOS.
- O que é do macOS fica isolado em `cfg(target_os = "macos")`: abrir arquivos pelo Finder
  (`platform.rs`), idioma pelo `AppleLocale` e um detalhe do modo de captura.
- No Linux o idioma vem de `LANG`/`LC_ALL`, e as preferências ficam em `~/.config/mermaidcraft`.

## O que muda

1. **Janela no Linux.** O eframe estava sem as features `wayland` e `x11`, e a janela não abria.
   Elas passam a valer só no Linux, com o winit carregando a biblioteca do Wayland em tempo de
   execução (`wayland-dlopen`) e desenhando a barra de título no GNOME (`wayland-csd-adwaita`).
2. **Identidade no Linux.** A janela usa o app id `ai.storyteller.mermaidcraft`, o mesmo do macOS,
   para o ambiente gráfico ligar a janela ao atalho `.desktop` e ao ícone.
3. **Preferências no Windows** em `%APPDATA%\MermaidCraft\settings.txt`. Antes dependiam de `HOME`,
   que o Windows não define fora do Git Bash e do MSYS: nada era lido nem gravado. No Linux passam
   a seguir `$XDG_CONFIG_HOME` quando ele é absoluto (o padrão continua `~/.config/mermaidcraft`).
4. **Sem console no Windows.** O app de release abre sem a janela preta de console. O CLI continua
   sendo de console. A troca: `MermaidCraft.exe --help`, `--version` e os erros de `--screenshot` não
   aparecem num terminal (quem chama por programa e lê a saída continua recebendo).
5. **Empacotamento:**
   - `packaging/linux/package.sh [--formats tar] [--install]`: compila e gera
     `dist/release/mermaidcraft-<versão>-linux-<arch>.tar.gz`, com os dois binários, o atalho
     `.desktop`, o ícone (SVG, e PNG quando há inkscape ou ImageMagick) e o tipo de arquivo `.mmd`.
     É a interface que o `craft-update` do Linux usa: extrai o tar em `~/.local` com
     `--strip-components=1`. Com `--install`, o próprio script faz isso e atualiza os bancos de
     atalhos, ícones e tipos de arquivo. O atalho, o tipo de arquivo e a base do script vêm da
     sessão do Fedora, onde foram validados (`desktop-file-validate`, `xdg-mime`, janela no Wayland
     do GNOME).
   - `packaging/windows/package.ps1`: compila e monta `target\windows-package\` com
     `MermaidCraft.exe`, `mermaidcraft-cli.exe`, licença e README, e um `.zip`.
6. **CI no GitHub Actions** (`.github/workflows/ci.yml`): compila, roda os testes e o clippy no
   Linux, no Windows e no macOS a cada push e pull request.
7. **Fim de linha fixo** (`.gitattributes`): o Git do Windows converte para CRLF por padrão, e os
   testes comparam textos byte a byte.
8. README com a instalação por sistema.

## Fora do escopo

- Instalador do Windows (MSI) e associação da extensão `.mmd` no Windows.
- Ícone dentro do `.exe` (exige uma dependência de build só para isso); a janela já mostra o ícone.
- Idioma automático no Windows (vem em inglês; a escolha em Exibir fica salva).
- Ensinar o `craft-update` a usar o script do Linux: é do repositório `craft`.

## Critérios de aceite

1. `cargo check` passa para `x86_64-unknown-linux-gnu` e `x86_64-pc-windows-msvc` a partir do Mac.
2. O CI fica verde nos três sistemas depois do push.
3. No Fedora, `packaging/linux/package.sh --install` instala, e o app abre com barra de título,
   aparece no menu de aplicativos e abre um `.mmd` pelo gerenciador de arquivos.
4. No macOS nada muda: o desenho exportado é idêntico, e testes e clippy passam.

## Revisão (19/11/2026)

Revisor independente, sem achado alto nem médio. Corrigidos:

- `package.sh`: falha do inkscape ou do ImageMagick derrubava o empacotamento sem mensagem (agora só
  avisa); a falta das ferramentas que atualizam menu, ícones e tipos de arquivo passava calada
  (agora avisa); o `PREFIX` herdado do ambiente decidia onde instalar (agora `--prefix`, padrão
  `~/.local`); `CARGO_TARGET_DIR` e `DIST` relativos apontavam para lugares diferentes no build e na
  cópia; versão vazia gerava um nome de arquivo quebrado; ImageMagick 6 (`convert`) não era
  usado; tarballs de versões antigas ficavam em `dist/release`; com `--install`, o atalho passa a
  chamar o binário instalado pelo caminho completo.
- `package.ps1`: `CARGO_TARGET_DIR` relativo; no PowerShell 5.1 com redirecionamento, a saída do
  cargo era tomada por erro.
- Preferências: `XDG_CONFIG_HOME` relativo é ignorado, como manda a especificação.
- README: a instalação manual do tar inclui os comandos que registram o tipo `.mmd`; a troca do
  app sem console no Windows ficou registrada. Comentário do `settings.rs` atualizado.

## Limites conhecidos

- O atalho instalado só pelo tar (como faz o `craft-update`) usa `Exec=mermaidcraft`: depende de
  `~/.local/bin` estar no PATH da sessão gráfica. No Fedora está; no Ubuntu, só depois de relogar
  se a pasta não existia.
- Quem usava o CLI no Linux com `XDG_CONFIG_HOME` apontando para outro lugar precisa mover o
  `settings.txt`; no Git Bash do Windows, de `%HOME%\.config\mermaidcraft` para `%APPDATA%`.
- Janela no Windows e `package.ps1` só são verificados pelo CI depois do push.
