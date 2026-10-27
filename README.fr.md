# Confucius Memory pour Codex

**Gardez le fil de votre travail.**

[English](README.md) · [Deutsch](README.de.md) · [Français](README.fr.md)

Une longue session de développement produit bien plus que du code : des
décisions, des impasses, des contraintes subtiles et la compréhension de ce qui
a finalement permis de résoudre un problème. Confucius Memory aide à conserver
ce contexte pour la suite de la conversation.

Ce petit compagnon écrit en Rust crée automatiquement des résumés de tâche et
des notes de projet ciblées pour Codex. L’agent principal n’a pas besoin de
rédiger des points de reprise. Son fonctionnement est adapté du planificateur
et du preneur de notes de Confucius, développé par Meta.

## Ce qu’il retient

- **La tâche :** les exigences, les corrections de l’utilisateur, les décisions
  et l’état d’avancement.
- **Les difficultés :** les approches infructueuses et les découvertes utiles
  pour la suite, accompagnées de références aux éléments qui les étayent.
- **La suite :** le travail restant et la première action concrète à entreprendre.

Chaque résumé appartient à une session. Les notes de projet sont accessibles
aux autres sessions du même répertoire de travail. Les worktrees et les
sous-répertoires disposent de mémoires distinctes.

## Son intégration à Codex

Les hooks natifs s’en chargent : `Stop` traite le nouvel historique et lance un
résumé lorsqu’un budget est atteint ; `PreCompact` résume les éléments en
attente avant la compression ; `SessionStart` réintroduit le résumé de la tâche
et un index des notes de projet.

Ce mécanisme complète la compression native de Codex. Il ne remplace pas son
algorithme et ne réécrit pas la conversation d’origine. Chaque résumé réussi
est enregistré avec la position exacte de l’historique qu’il couvre. Si un appel
ultérieur dépasse le délai prévu, le traitement peut reprendre à partir du
progrès enregistré. Sans nouvel historique, aucun nouvel appel au modèle n’est lancé.

## Premiers pas

Il vous faut **Rust 1.88 ou ultérieur**, `make` et une CLI Codex authentifiée,
compatible avec les hooks natifs et les options du worker. Testé avec
**Codex CLI 0.161.0 sur macOS**.

```sh
git clone https://github.com/itslonua/codex-confucius-memory.git
cd codex-confucius-memory
make check
make build
./target/release/codex-memory --help
```

Générez les entrées des hooks avec le binaire de production :

```sh
./target/release/codex-memory --codex /absolute/path/to/codex hooks > /tmp/confucius-hooks.json
```

Vérifiez le fichier généré, puis ajoutez ses entrées au fichier
`.codex/hooks.json` de votre projet, en conservant les hooks existants. Dans
Codex, utilisez `/hooks` pour examiner et approuver les entrées. La configuration
du projet doit également être reconnue comme fiable. Conservez le binaire à
l’emplacement utilisé lors de la génération.

Le [skill de mémoire fourni](skills/confucius-memory/SKILL.md) aide Codex à
retrouver les découvertes enregistrées avant de recommencer une investigation.
`search` trouve les notes correspondantes et `read` lit une note, sans appel au
modèle. Le guide explique aussi comment installer le skill.

Le [guide d’utilisation en anglais](docs/usage.md) détaille la configuration,
les budgets, les commandes manuelles, le dépannage et la désinstallation.

## Données et utilisation du modèle

La mémoire est stockée localement dans `$CODEX_HOME/confucius-memory`, ou dans
`$HOME/.codex/confucius-memory` si `CODEX_HOME` n’est pas défini. Les nouveaux
répertoires ont des permissions privées. Les instantanés sont remplacés de
manière atomique et un verrou par projet évite les écritures concurrentes.

**La création des résumés envoie une sélection de l’historique et des notes au
service de modèle utilisé par le worker Codex CLI isolé.** Celui-ci utilise
votre authentification Codex existante et votre quota habituel. Il n’hérite pas
de la configuration utilisateur personnalisée, y compris des paramètres de
fournisseur personnalisés. Ces appels peuvent ajouter de la latence ; les
résumés ne sont pas produits hors ligne.

## État du projet et limites

**Version expérimentale 0.1.** Un compagnon pratique pour les longues sessions,
avec des tests couvrant le traitement incrémental, la reprise, l’isolation des
données et les erreurs de sous-processus. Les résumés perdent des détails et
peuvent être incomplets ou périmés. Vérifiez les informations importantes dans
la conversation d’origine et le code. Le format des rollouts Codex n’est pas une API stable.

Aucune économie de tokens ni amélioration sur des tâches de longue durée par
rapport à la compression native de Codex n’a été démontrée. L’objectif est de
préserver le contexte utile ; le bénéfice réel dépend de la tâche et de la
qualité des résumés.

## Crédits

Adapté de [Confucius de Meta](https://github.com/facebookresearch/cca-swebench/tree/main/confucius).
Consultez la [provenance et les modifications](THIRD_PARTY_NOTICES.md) ainsi que
la [licence MIT d’origine](LICENSE-META). Ce projet communautaire n’est affilié
ni à Meta ni à OpenAI.
