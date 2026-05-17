# Pruebas LR(1)

```bash
cargo run --bin test_lr1
```

Cuando pida la ruta del archivo:

| Gramática | Ruta |
|---|---|
| Ejemplo del documento | `examples/basic/grammar_lr1.yalp` |
| Gramática de C | `examples/basic/ejemplo_c.yalp` |

**Cadenas para probar con `grammar_lr1.yalp`:**

| Cadena | Resultado |
|---|---|
| `d d` | válida |
| `c d c d` | válida |
| `c c d c c d` | válida |
| `c d` | inválida |
| `c d c` | inválida |

**Release (gramáticas grandes):**
```bash
cargo build --release --bin test_lr1 && ./target/release/test_lr1
```
