# Confucius Memory für Codex

**Behalte den roten Faden deiner Arbeit.**

[English](README.md) · [Deutsch](README.de.md) · [Français](README.fr.md)

In langen Coding-Sitzungen entsteht mehr als Code: Entscheidungen, Sackgassen,
knifflige Randbedingungen und das Wissen, warum eine Lösung schließlich
funktioniert hat. Confucius Memory hilft, diesen Kontext in die nächste Phase
des Gesprächs mitzunehmen.

Der kleine Rust-Begleiter für Codex erstellt automatisch Zusammenfassungen zur
Aufgabe und ausgewählte Projektnotizen. Der Hauptagent muss keine Checkpoints
schreiben. Der Ansatz adaptiert Metas Confucius-Planner, Note-Taker und
hierarchischen Speicher. Notizen liegen als Markdown-Dateien nach Themen geordnet vor.

## Was erhalten bleibt

- **Die Aufgabe:** Anforderungen, Korrekturen des Nutzers, Entscheidungen und
  der aktuelle Arbeitsstand.
- **Die schwierigen Stellen:** gescheiterte Ansätze und Erkenntnisse, die sich
  für spätere Arbeiten lohnen — mit Verweisen auf die zugehörigen Belege.
- **Der nächste Schritt:** offene Arbeit und die erste konkrete nächste Aktion.

Zusammenfassungen gehören jeweils zu einer Sitzung. Projektnotizen stehen auch
anderen Sitzungen im selben Arbeitsverzeichnis zur Verfügung. Verschiedene
Worktrees und Unterverzeichnisse haben getrennte Speicherbereiche.

## So arbeitet es mit Codex zusammen

Native Hooks übernehmen die Arbeit: `Stop` verarbeitet neue Gesprächsinhalte und
fasst sie beim Erreichen eines Budgets zusammen. `PreCompact` fasst ausstehende
Inhalte vor der Komprimierung zusammen. `SessionStart` stellt die Zusammenfassung
und ein Verzeichnis der Projektnotizen bereit.

Das ergänzt die eingebaute Kontextkomprimierung von Codex. Ihr Algorithmus und
das ursprüngliche Gespräch bleiben erhalten. Jede erfolgreiche Zusammenfassung
wird mit der genauen Position der verarbeiteten Gesprächshistorie gespeichert.
Nach einem späteren Timeout lässt sich dort fortsetzen. Ohne neue Inhalte wird
kein weiterer Modellaufruf ausgelöst.

## Einstieg

Benötigt werden **macOS oder Linux, Rust ab 1.88**, `make` und eine angemeldete Codex CLI mit
passenden nativen Hooks und Worker-Optionen. Getestet mit **Codex CLI 0.161.0
unter macOS**.

```sh
git clone https://github.com/itslonua/codex-confucius-memory.git
cd codex-confucius-memory
make check
make build
./target/release/codex-memory --help
```

Erzeuge die Hook-Einträge mit dem Release-Binary:

```sh
./target/release/codex-memory --codex /absolute/path/to/codex hooks > /tmp/confucius-hooks.json
```

Prüfe die erzeugte Datei und ergänze ihre Einträge in der `.codex/hooks.json`
deines Projekts. Bestehende Hooks bleiben dabei erhalten. Prüfe und bestätige
die neuen Einträge in Codex über `/hooks`; auch die Projektkonfiguration muss
als vertrauenswürdig eingestuft sein. Das Binary muss anschließend unter dem
Pfad verfügbar bleiben, der bei der Erzeugung verwendet wurde.

Der mitgelieferte [Memory-Skill](skills/confucius-memory/SKILL.md) hilft Codex,
gespeicherte Erkenntnisse vor einer erneuten Untersuchung zu finden. `search`
sucht nach Wörtern oder nach Pfad, Inhalt und Tags. `read` öffnet Notizen,
Zeilenbereiche oder Verzeichnisse, ohne Modellaufruf.
Die Anleitung erklärt auch die Installation des Skills.

Die [Anleitung auf Englisch](docs/usage.md) beschreibt Konfiguration, Budgets,
manuelle Befehle, Fehlerbehebung und Deinstallation.

## Daten und Modellnutzung

Der Speicher liegt lokal unter `$CODEX_HOME/confucius-memory`, andernfalls unter
`$HOME/.codex/confucius-memory`. Neue Verzeichnisse erhalten private Zugriffsrechte.
Zusammenfassungen und Notizen sind Markdown-Dateien; Fortsetzungsdaten bleiben
getrennt. Eine Projektsperre und ein Wiederherstellungsjournal sichern Änderungen.
Bestehende JSON-Speicher werden beim ersten Schreiben migriert; die Originaldatei bleibt erhalten.

**Für Zusammenfassungen werden ausgewählte Gesprächsinhalte und Notizen an den
Modelldienst des isolierten Codex-CLI-Workers gesendet.** Dabei gelten deine
bestehende Codex-Anmeldung und dein reguläres Nutzungskontingent. Der Worker
übernimmt keine benutzerdefinierte Konfiguration, auch keine eigenen
Provider-Einstellungen. Modellaufrufe können zusätzliche Wartezeit verursachen;
die Zusammenfassung erfolgt nicht offline.

## Stand und Grenzen

**Experimentelle Version 0.1.** Ein praktischer Begleiter für lange Sitzungen,
mit Tests für schrittweise Verarbeitung, Wiederaufnahme, Trennung der Daten und
Fehler in Unterprozessen. Zusammenfassungen verlieren Details und können
unvollständig oder veraltet sein. Prüfe wichtige Aussagen anhand des
ursprünglichen Gesprächs und des Codes. Das Codex-Rollout-Format ist keine stabile API.

Eine Token-Ersparnis oder bessere Ergebnisse bei Langzeitaufgaben gegenüber der
nativen Codex-Komprimierung sind bisher nicht nachgewiesen. Ziel ist es,
nützlichen Kontext zu erhalten; der tatsächliche Nutzen hängt von der Aufgabe
und der Qualität der Zusammenfassungen ab.

## Herkunft

Adaptiert aus [Metas Confucius](https://github.com/facebookresearch/cca-swebench/tree/main/confucius).
Siehe [Herkunft und Änderungen](THIRD_PARTY_NOTICES.md) sowie die
[MIT-Lizenz des Originals](LICENSE-META). Ein Community-Projekt ohne Verbindung
zu Meta oder OpenAI.
