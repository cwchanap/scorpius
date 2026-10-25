# Regional terrain art

`terrain.png` is built by `tools/terrain_atlas.py` from source art generated with the Codex CLI's built-in image generation on 2026-10-25 (codex-cli 0.163.0). The source images are not checked in; a copy lives in the project files under `map-art/src/`. Rebuild with:

```bash
python3 tools/terrain_atlas.py path/to/terrain-src assets/ui/terrain.png --preview target/terrain-preview.png
```

The script prints each terrain's rim colour; keep `theme::terrain_rim` in step with it.

## Layout

All geometry is mirrored in `src/presentation/theme.rs`.

- Tiles are painted at 2x stage pixels. Each slot is 212x180: the cell inset's 106x50 ground diamond sits at the bottom, and forests (74px) and mountains (80px) rise above it.
- One row per terrain, in order Plain, Road, Forest, Sea, Mountain. Each row holds 16 slots, one per cell of a repeating 4x4 period (`(y % 4) * 4 + x % 4`), so neighbouring cells show neighbouring pieces of one continuous ground texture.
- Below the rows, at (0, 900), is the 224x164 blocking-wall prism: the full 112x56 cell raised 26px.

## How the art is made

- Ground is projected from seamless top-down textures onto the 2:1 diamond (a narrow cross-fade fixes any seam the generator left). Each texture is graded down in contrast so a field reads as one calm surface.
- Forest tiles plant four to five trees, cut from the two tree sprite sheets, on the forest-floor texture with soft shadows. Every slot has its own arrangement.
- Mountain tiles place one massif from the mountain sheets on the rock texture, stretched 1.2x taller and jittered in size and position. Only three of the sixteen slots get a snow cap.
- The wall prism maps the concrete texture onto a top face and two shaded side faces with a hazard-stripe band.

## Generation prompts

Every prompt ended with: "Clean hand-painted anime game art in the style of classic Super Robot Wars / Japanese mecha tactics RPG battle maps: soft cel shading, bright clear colors, light from the upper left. No text, no labels, no watermark."

Textures (requested at 1024x1024, returned at 1254x1254; top-down orthographic, wrapping seamlessly, even flat lighting, low contrast):

- `grass-1.png` (used; `grass-2.png` was the alternative take): "Seamless tileable top-down ground texture for a mecha tactics RPG battle map: meadow grass in fresh bright medium green, built from large soft clumps of slightly lighter and darker green, with a few tiny pale-yellow flower specks. No paths, no rocks, no objects, no shadows."
- `sea-2.png` (used; `sea-1.png` was the alternative take): "Seamless tileable top-down water texture for a mecha tactics RPG battle map: open sea in clear saturated ocean blue shading toward teal, with soft stylized pale wave-crest strokes scattered evenly and a few gentle sparkles. Evenly distributed pattern, no directional gradient, no shoreline, no foam bands, no objects."
- `road.png`: "Seamless tileable top-down ground texture for a mecha tactics RPG battle map: military supply road surface of compacted light tan-gray gravel and dirt with fine pebbles and subtle darker patches, warm neutral color. No tire tracks, no lane lines, no directional marks, no objects."
- `forest-floor.png`: "Seamless tileable top-down ground texture for a mecha tactics RPG battle map: forest floor under a canopy, deep mossy green grass with darker shaded patches, a few small ferns and fallen leaves; darker and cooler than open meadow. No trees, no objects."
- `rock.png`: "Seamless tileable top-down ground texture for a mecha tactics RPG battle map: rocky highland ground of gray-brown stone slabs and scree with patches of dry yellow-green moss. No objects."
- `concrete.png`: "Seamless tileable top-down texture for a mecha tactics RPG battle map: military bunker roof of medium cool-gray reinforced concrete slab panels in a square grid, thin dark steel seams, a few rivets, light weathering stains."

Sprite sheets (1536x1024, transparent background; both takes of each are used):

- `trees-1.png`, `trees-2.png`: "Game sprite sheet on a fully transparent background: 12 separate stylized trees for an isometric mecha tactics RPG battle map, seen from a high three-quarter isometric angle (camera about 55 degrees above the ground) so the round canopy tops are visible. 6 round broadleaf trees and 6 conical pine trees, varied sizes, rich greens from deep forest green to fresh leaf green, bright highlights on the upper left and darker shading on the lower right, short brown trunks visible under the canopy. Arranged in a 4 by 3 grid with wide empty space around every tree: no tree touches another tree or the image edge. No ground, no grass, no cast shadows."
- `mountains-1.png`, `mountains-2.png`: "Game sprite sheet on a fully transparent background: 4 separate stylized rocky mountains for an isometric mecha tactics RPG battle map, each one craggy mountain massif with gray-brown rock faces showing clear light and shadow planes, green mossy and grassy foothills around its base, a small snow cap on two of them. Viewed from a high three-quarter isometric angle; each mountain's base footprint is a wide flattened shape about twice as wide as it is deep, and each mountain is about two-thirds as tall as it is wide. Arranged in a 2 by 2 grid with wide empty space around each mountain; nothing touches another mountain or the image edge. No ground plane, no cast shadows."

The Codex command that worked, with the prompt on stdin, asking it to use the built-in `image_gen` tool and copy the PNG into the source folder:

```bash
codex exec --skip-git-repo-check -s workspace-write -C <repo> -o LASTMSG.txt -
```
