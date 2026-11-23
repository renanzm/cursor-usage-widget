# Painel de controle (configurações) — design

Data: 2026-11-23
Status: aprovado

## Objetivo

Permitir que o usuário ajuste, por uma janela de Configurações, os parâmetros que hoje só existem no `.env` ou estão fixos no código: cota mensal, janela de cálculo do ritmo, escala do velocímetro, limites de baixo/médio/alto, escala do equalizador, frequência de atualização e cores. As mudanças valem na hora, sem reiniciar o app.

Este é o primeiro de dois subprojetos. O segundo (suporte ao Claude Code, com custo estimado a partir de `~/.claude/projects/*.jsonl`) depende deste para a cota manual e terá spec próprio.

## Fora do escopo

- Percentuais de alerta do ciclo (75% e 90%) e a cor da barra do ciclo: continuam fixos.
- Cores de fundo e de texto.
- Pré-visualização ao vivo das cores antes de salvar.
- Cookie da sessão: continua no Credential Manager, fora do painel.

## Campos

| Grupo | Campo (`settings.json`) | Padrão | Validação |
|---|---|---|---|
| Cota | `quotaMode` | `"auto"` | `"auto"` (limite informado pelo Cursor) ou `"manual"` |
| Cota | `quotaUsd` | 1000 | > 0; usado só em `"manual"` |
| Ritmo | `windowHours` | 3 | 0,5 ≤ valor ≤ 24 |
| Ritmo | `rateMax` (fim da escala, US$/h) | 20 | > `highMin` |
| Ritmo | `lowMax` (baixo até, US$/h) | 6 | 0 < valor < `highMin` |
| Ritmo | `highMin` (alto a partir de, US$/h) | 13 | < `rateMax` |
| Semana | `dayMaxUsd` | 100 | ≥ 1 |
| Geral | `pollSeconds` | 30 | 10 ≤ valor ≤ 3600 |
| Cores | `colors.accent` | `#ff9f0a` | `#rrggbb` |
| Cores | `colors.low` | `#30d158` | `#rrggbb` |
| Cores | `colors.mid` | `#ffd60a` | `#rrggbb` |
| Cores | `colors.high` | `#ff453a` | `#rrggbb` |

Todos os campos numéricos precisam ser finitos.

Classificação do nível (inalterada): baixo se `ritmo ≤ lowMax`; médio se `ritmo < highMin`; alto caso contrário. O ritmo é `gasto da janela / windowHours`.

Cota efetiva: em `"auto"`, o limite informado pelo Cursor (ou `quotaUsd` se o Cursor não informar); em `"manual"`, sempre `quotaUsd`.

## Arquitetura

### Rust (`src-tauri/src`)

- **`settings.rs` (novo).** Struct `Settings` com `serde` (`camelCase`, `#[serde(default)]` em todos os campos), `Default` com os valores da tabela, `validate() -> Result<(), Vec<FieldError>>` (`FieldError { field, message }`), `load(path)` e `save(path)`. Arquivo em `app_config_dir()/settings.json`. Se o arquivo não existir, os valores iniciais vêm do `.env` (`CYCLE_LIMIT_USD`, `RATE_LOW_MAX`, `RATE_HIGH_MIN`, `RATE_MAX`, `DAY_MAX_USD`, `POLL_SECONDS`, via `config::setting_f64`) e, na falta deles, dos padrões; o resultado é gravado.
- **`config.rs`.** Fica com o cookie e com a leitura do `.env` usada na importação inicial.
- **`AppState`.** Ganha `settings: Mutex<Settings>` e o caminho do arquivo.
- **`usage.rs`.** `fetch` e `Snapshot::empty` recebem `&Settings` em vez de ler o `.env`. `WINDOW_HOURS` sai; a janela vem de `windowHours`. A cota efetiva segue a regra acima. A classificação do nível vira função pura `level_of(rate, low_max, high_min) -> &'static str`. `Snapshot` ganha `window_hours`.
- **`alerts.rs`.** A notificação de ritmo alto diz "nas últimas {windowHours}h".
- **`main.rs`.**
  - `poll_loop` relê `pollSeconds` a cada volta.
  - Comandos novos: `get_settings`; `save_settings(settings)` (valida → grava → atualiza o estado → emite o evento `settings` para todas as janelas → `notify_one()` para consultar de novo; em falha de validação devolve a lista de `FieldError`, em falha de gravação devolve a mensagem); `open_settings` (cria a janela `settings` ou a traz para a frente se já existir).
  - Menu da bandeja: item "Configurações…" que chama a mesma rotina de `open_settings`.
- **`capabilities/default.json`.** Incluir a janela `settings`.

### Interface (`ui/`)

- **`settings.html`, `settings.js`, `settings.css` (novos).** Janela normal com barra de título do Windows, ~420×640, visual preto/cinza do widget. Seções Cota, Ritmo, Semana, Geral e Cores. Botões: Salvar, Cancelar, "Restaurar padrão" na seção Cores (só cores) e "Restaurar tudo" no rodapé (preenche o formulário com os padrões; só grava ao Salvar). Erros de validação aparecem ao lado de cada campo, em vermelho. O campo de cota manual fica desabilitado em modo Automático.
- **`app.js`.** Carrega as configurações ao abrir e escuta o evento `settings`; aplica as cores como variáveis CSS no elemento raiz. Ícone de engrenagem no painel expandido chama `open_settings`. Textos com "3h" passam a usar `windowHours` do snapshot.
- **`style.css`.** Variáveis `--orange`, `--lvl-low`, `--lvl-mid`, `--lvl-high` definidas pelo JS. O fundo translúcido de cada nível passa a ser `color-mix(in srgb, var(--lvl-x) 16%, transparent)`. As sombras e brilhos hoje escritos como `rgba(255, 159, 10, …)` passam a derivar de `--orange` do mesmo jeito. A alteração local ainda não commitada em `style.css` deve ser preservada.

### Fluxo

Salvar → `save_settings` valida e grava → Rust emite `settings` e força nova consulta → o widget troca as cores imediatamente e, na sequência, recebe o novo `usage` com nível, escala, janela e cota recalculados.

## Erros

- `settings.json` corrompido ou com valor inválido: campos ilegíveis ou fora da regra assumem o padrão; os demais são mantidos. O app não trava.
- Falha ao gravar: o painel mostra a mensagem e mantém os valores digitados.
- Cor fora do formato `#rrggbb`: o seletor impede; o Rust valida de novo.
- Cancelar ou fechar a janela: descarta as alterações.

## Testes

- Unitários em `settings.rs` e `usage.rs` (primeiros testes do projeto):
  - cada regra de validação, incluindo limites (`windowHours` 0,5 e 24, `lowMax == highMin` inválido, `highMin == rateMax` inválido);
  - `load` com arquivo incompleto e com JSON corrompido;
  - cota efetiva em `"auto"` (com e sem limite do Cursor) e em `"manual"`;
  - `level_of` com limites personalizados nas fronteiras.
- Manual de ponta a ponta (estilo `tools/e2e-paste.ps1`): abrir Configurações pela bandeja, mudar cor de destaque e limites, salvar, conferir com `tools/shot.ps1` que o widget mudou na hora, reiniciar e confirmar que os valores persistiram.
- Verificação final: `cargo build`, `cargo test`, `cargo run -- --once` (snapshot com `windowHours` e cota efetiva).
