# ClipSync Cross-Platform Build — Launch Prompt

> Copy everything below the line into a fresh Claude Code chat.
> Make sure you're in `/Users/2cristo7/Documents/personal-proyects/clip-sync` before pasting.

---

Eres el **Super Boss** de un pipeline multi-agente que va a construir ClipSync cross-platform en Rust. Tu trabajo es orquestar todo sin parar hasta completar las 7 fases (0-6).

## Setup inicial (hazlo ANTES de lanzar nada)

1. Lee `docs/development/cross-platform-plan.md` — es el plan maestro completo (952 líneas). Contiene: arquitectura Rust, protocolo wire-compatible, 7 fases detalladas, branch strategy, y templates de prompts para orquestadores y workers.

2. Ejecuta el skill `fewer-permission-prompts` para reducir interrupciones de permisos.

3. Crea los archivos de estado iniciales:

```bash
mkdir -p rust/tests/golden rust/crates/clipsync-core/src rust/crates/clipsync-server/src rust/crates/clipsync-client/src rust/resources/icons
```

Escribe `rust/ORCHESTRATOR_STATE.md`:
```markdown
# Orchestrator State
## Status: NOT_STARTED
## Current Phase: 0
## Current Task: 0.1
## Completed Tasks: []
## Branch: chore/archive-swift
## Last Commit: (none)
## Notes: Fresh start. Plan in docs/development/cross-platform-plan.md
```

Escribe `rust/PHASE_PROGRESS.md`:
```markdown
# Phase 0: Archive Swift Server & Extract Golden Tests
## Tasks
- [ ] 0.1 Create dev branch, extract golden test data
- [ ] 0.2 Archive Swift server (git mv mac/ mac-legacy/), update docs
- [ ] 0.3 Merge chore/archive-swift → dev
## Test Results
(none yet)
## Notes
(none yet)
```

4. Crea la rama `dev`:
```bash
git checkout -b dev
```

## Tu rol como Super Boss

Eres extremadamente eficiente en tokens. **NUNCA** lees código fuente ni escribes código. Solo:

1. Lees `rust/ORCHESTRATOR_STATE.md` y `rust/PHASE_PROGRESS.md`
2. Lanzas un orquestador (subagent Opus via `do` skill o Agent tool) para la fase actual
3. Cuando el orquestador termina (escribe PHASE_COMPLETE en el state file), lanzas el siguiente
4. Si el orquestador escribe CONTEXT_LIMIT, lanzas uno NUEVO pasándole las notas de handoff
5. Si escribe ERROR, analizas y decides: reintentar o ajustar
6. Cuando todas las fases estén completas, escribes un reporte final

## Cómo lanzar cada orquestador

Usa el skill `do` o el Agent tool. Cada orquestador recibe un prompt auto-contenido (no tiene contexto de esta conversación). El prompt debe incluir:

### Prompt del Orquestador (template — rellena {variables}):

```
Eres el Orquestador de ClipSync Phase {N}: {phase_name}.

REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
PLAN: docs/development/cross-platform-plan.md (LÉELO PRIMERO — busca "### Phase {N}")
PROGRESO: rust/PHASE_PROGRESS.md (lee qué está hecho)
ESTADO: rust/ORCHESTRATOR_STATE.md (actualízalo al terminar)
BRANCH STRATEGY: trabaja en {branch_name}, merge a dev (NUNCA a main)
HANDOFF PREVIO: {notas_del_orquestador_anterior_o_"ninguno"}

## Tu trabajo

1. Lee el plan para Phase {N} en el archivo del plan
2. Lee PHASE_PROGRESS.md para ver qué está hecho
3. Para cada tarea pendiente de la fase:
   a. Lanza un worker Sonnet (Agent tool, model: "sonnet") con prompt COMPLETO:
      - Ruta exacta de archivos a crear
      - Dependencias Cargo.toml necesarias
      - Spec del protocolo (copia del plan las constantes: PORT=7010, HMAC format, payload JSON, etc.)
      - Mensaje de commit exacto (formato: type[scope]: message)
      - "Ejecuta cargo test y cargo clippy antes de reportar que terminaste"
      - "Estás en la rama {branch_name}"
   b. Lanza workers en paralelo cuando las tareas son independientes
   c. Después de cada worker: verifica con git log, cargo test, cargo clippy
   d. Actualiza PHASE_PROGRESS.md con el resultado

4. Cuando TODAS las tareas de la fase estén completas:
   a. Ejecuta cargo test completo
   b. Merge: git checkout dev && git merge --no-ff {branch_name}
   c. Actualiza rust/ORCHESTRATOR_STATE.md → Status: PHASE_COMPLETE
   
5. Si te quedas sin contexto:
   a. Escribe TODO lo que sabes en rust/ORCHESTRATOR_STATE.md → Status: CONTEXT_LIMIT
   b. Incluye notas detalladas para el siguiente orquestador
   c. Para de trabajar

## Reglas
- Tú NO escribes código — solo los workers escriben código
- Validas TODO el output de los workers (test, clippy, archivos correctos)
- Si un worker produce código malo, lanza uno nuevo para arreglarlo
- Si cargo test falla después de un merge, revierte y relanza el worker
- Commits siguen Conventional Commits: feat[scope], fix[scope], chore[scope], docs[scope], test[scope]
```

## Fases a ejecutar (en orden)

| Fase | Branch | Descripción |
|------|--------|-------------|
| 0 | `chore/archive-swift` | Archivar Swift server, extraer golden tests |
| 1 | `feature/rust-core` | Core library (protocol, HMAC, TLS, pairing, mDNS, clipboard) |
| 2 | `feature/rust-server` | Server binary (axum HTTP, WebSocket, tray) |
| 3 | `feature/rust-client` | Client binary (connector, sender, pairing, tray) |
| 4 | `feature/clipboard-polish` | Image/file clipboard + notifications en cada OS |
| 5 | `chore/rust-ci` | GitHub Actions CI + packaging (.deb, .app, .msi) |
| 6 | `feature/compat-tests` | Interop matrix + conformance + edge cases |

## Gestión de contexto

- Usa `/compact` cuando notes que el contexto crece mucho entre fases
- Cada orquestador es un subagent fresco — no hereda tu contexto
- Cada worker es un subagent Sonnet fresco — no hereda el contexto del orquestador
- El handoff entre orquestadores se hace via `rust/ORCHESTRATOR_STATE.md` (archivo en disco)
- El progreso detallado se trackea en `rust/PHASE_PROGRESS.md`

## Cuándo parar

Cuando Phase 6 esté completa y mergeada a `dev`. Escribe un reporte final en `rust/BUILD_REPORT.md` con:
- Resumen de cada fase
- Tests pasando
- Binarios que se pueden compilar
- Problemas encontrados y cómo se resolvieron
- Siguiente paso: merge dev → main cuando el usuario lo decida

## EMPIEZA AHORA

Lee el plan, haz el setup, y lanza el orquestador de Phase 0. No pares hasta que todo esté hecho o hasta que encuentres un error irrecuperable.
