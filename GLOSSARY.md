# Glossary (Repo Excerpt)

For the full glossary, see: https://github.com/instance001/Whatisthisgithub/blob/main/GLOSSARY.md

This file contains only the glossary entries for this repository. Mapping tag legends and global notes live in the full glossary.

## chatty-quest
| Term | Alternate term(s) | Alt map | External map | Relation to existing terminology | What it is | What it is not | Source |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Chatty Quest | chatty-quest | = | ~ | Deterministic desktop adventure game | First game built on the RD Engine, presented as a chat-forward Dungeon Master experience over deterministic state and reducer-confirmed mutations | Not a freeform LLM-only narrative toy; not nondeterministic core game logic | chatty-quest/README.md |
| RD Engine | Radiant Determinism Engine | = | ~ | Deterministic adventure engine | Rust desktop adventure engine where deterministic templates and bucketed state create the world and a narrator layer presents it conversationally | Not a physics engine; not a pure language-model game engine | chatty-quest/README.md |
| Radiant Determinism | radiant determinism | = | ~ | Deterministic-reactive design stance | Design stance where the experience can feel dynamic and alive while meaningful gameplay payoff remains grounded in deterministic state, reducers, and visible UI updates | Not hidden random story steering; not unverifiable game truth | chatty-quest/README.md |
| Templates / buckets / reducers / narration / media | engine grammar | ~ | ~ | Engine component grammar | Repo shorthand where templates are the nouns, buckets the current grammar, reducers the verbs, narration the accent, and media the illustration | Not five unrelated subsystems; not a claim that narration owns game truth | chatty-quest/README.md |
| MockNarrator | mock narrator | = | ~ | Replaceable narrator seam | Current or initial narrator layer used to prove that narration can be swapped while reducer-owned state remains authoritative | Not the source of deterministic world state; not required to stay permanent | chatty-quest/README.md |
| Property Siege Classic | scenario pack | ~ | ~ | Initial playable pack | The one playable scenario pack used in v0.1 to prove datapack-driven loading, deterministic run state, reducer actions, and save/load reliability | Not the entire future content scope of RD Engine | chatty-quest/README.md |
