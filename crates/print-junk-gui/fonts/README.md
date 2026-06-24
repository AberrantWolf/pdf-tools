# Embedded fonts

These fonts are embedded into the GUI binary via `include_bytes!`. All are
licensed under the **SIL Open Font License 1.1**, which explicitly permits
bundling fonts inside software — free or commercial — as long as the copyright
notice and license are included (see the `OFL-*.txt` files here). The fonts are
embedded **unmodified under their original names**, so the Reserved Font Names
("Plex", etc.) impose no extra obligation.

| Font | Role in the UI | Copyright | License file |
|------|----------------|-----------|--------------|
| Noto Sans (Regular) | UI body / proportional text | The Noto Project Authors | `OFL-Noto.txt` |
| Noto Sans Symbols2 (Regular) | symbol/glyph fallback | The Noto Project Authors | `OFL-Noto.txt` |
| IBM Plex Serif (SemiBold) | headings / wordmark | IBM Corp. | `OFL-IBMPlex.txt` |
| IBM Plex Mono (Regular) | measurement values (tabular) | IBM Corp. | `OFL-IBMPlex.txt` |

Sources: [Noto](https://github.com/notofonts), [IBM Plex](https://github.com/IBM/plex),
retrieved via [google/fonts](https://github.com/google/fonts).
