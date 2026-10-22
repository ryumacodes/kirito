# Bundled runtime provenance

The runtime archive is distributed at the project owner's explicit direction
under their stated permission to redistribute Metal with Kirito.

- Wine host / D3DMetal 3 pairing: GCenX Game Porting Toolkit 3.0-3,
  https://github.com/Gcenx/game-porting-toolkit/releases/tag/Game-Porting-Toolkit-3.0-3
- Original download SHA-256:
  `d377683937340f914823dbb2e1252b329cbf834ff58907d0293db8cebf0e392e`.
- The Apple framework's LICENSE is preserved inside the archive at
  `d3dmetal-redist/external/D3DMetal.framework/Versions/A/Resources/LICENSE`.
- Third-party components retain their respective licenses. The publisher's
  permission does not relicense them as Kirito source.

This archive is the tested general runtime. Bedrock uses a separately downloaded
WineForge/GPTK 4 pairing; passing device tests does not establish game compatibility.
