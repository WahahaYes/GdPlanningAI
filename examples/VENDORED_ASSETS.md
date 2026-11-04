# Vendored Assets

Third-party asset packs used by the examples. Each pack lives under `source_2d/assets/vendored/<PackName>/` (or `source_3d/...` for 3D), with its original internal layout preserved. License copies live in `examples/licenses/`.

| Pack                                             | Used for                                                                          | Source                                                                                  | License                                                                                    |
| ------------------------------------------------ | --------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| Tiny Swords by Pixel Frog (Free Pack)            | 2D agent sprites (`Units/`), tileset and decorations (`Terrain/`, `Decorations/`) | https://pixelfrog-assets.itch.io/tiny-swords                                            | [licenses/TinySwordsPack-LICENSE.md](licenses/TinySwordsPack-LICENSE.md)                   |
| KayKit Adventurers 2.0 FREE by Kay Lousberg      | 3D agent characters and animations                                                | https://www.kaykit.com                                                                  | [licenses/KayKit_Adventurers-LICENSE.txt](licenses/KayKit_Adventurers-LICENSE.txt)         |
| KayKit Resource Bits 1.0 FREE by Kay Lousberg    | 3D campfire logs, wood piles, held wood, rocks, barrels and ruins                 | https://www.kaykit.com                                                                  | [licenses/KayKit_ResourceBits-LICENSE.txt](licenses/KayKit_ResourceBits-LICENSE.txt)       |
| KayKit Medieval Hexagon Pack 1.0 by Kay Lousberg | 3D tree line, hills, mountains and clouds around the play area; campfire stones   | https://github.com/KayKit-Game-Assets/KayKit-Medieval-Hexagon-Pack-1.0 (commit 84fa4e9) | [licenses/KayKit_MedievalHexagon-LICENSE.txt](licenses/KayKit_MedievalHexagon-LICENSE.txt) |
| MedievalSharp font by wmk69                      | 3D debug text: labels over agents and objects, the controls guide                 | https://github.com/google/fonts/tree/main/ofl/medievalsharp                             | [licenses/MedievalSharp-OFL.txt](licenses/MedievalSharp-OFL.txt)                           |

Only the Tiny Swords Free Pack (blue-ribbon content: human units, faction colors, terrain, decorations) is used. The paid Enemy Pack is not vendored.

The KayKit packs are CC0 (see their license copies, shipped as `License.txt` inside each pack). Only the nature models the backdrop and campfire use are vendored from the Medieval Hexagon Pack, under their original `Assets/gltf/decoration/nature/` path. MedievalSharp is under the SIL Open Font License 1.1 (`OFL.txt` beside the font).
