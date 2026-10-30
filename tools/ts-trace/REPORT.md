# Recorded TS fixtures

63 fixtures, 8597772 bytes. Two independent recordings compared byte for byte before writing.

| Fixture | Ticks | Bytes |
| --- | ---: | ---: |
| brawler-ground | 513 | 96267 |
| brawler-aerial-movement | 756 | 119455 |
| brawler-specials | 339 | 60761 |
| brawler-throw-f | 147 | 22697 |
| brawler-throw-b | 147 | 23104 |
| brawler-throw-u | 147 | 21544 |
| brawler-throw-d | 147 | 22735 |
| brawler-shield-hit | 53 | 9870 |
| swordfighter-ground | 538 | 101598 |
| swordfighter-aerial-movement | 756 | 119474 |
| swordfighter-specials | 408 | 77678 |
| swordfighter-throw-f | 147 | 22707 |
| swordfighter-throw-b | 147 | 23114 |
| swordfighter-throw-u | 147 | 21554 |
| swordfighter-throw-d | 147 | 22745 |
| swordfighter-shield-hit | 53 | 9891 |
| king-ground | 605 | 114229 |
| king-aerial-movement | 752 | 118643 |
| king-specials | 414 | 79982 |
| king-throw-f | 147 | 22691 |
| king-throw-b | 147 | 23098 |
| king-throw-u | 147 | 21538 |
| king-throw-d | 147 | 22729 |
| king-shield-hit | 53 | 9833 |
| ledge-climb | 128 | 16783 |
| ledge-attack | 128 | 19963 |
| ledge-roll | 128 | 16666 |
| ledge-jump | 128 | 20881 |
| ledge-drop | 128 | 16540 |
| walk-off-ledge | 122 | 15905 |
| brawler-uspec-ledge | 121 | 15666 |
| brawler-pound-air | 141 | 18502 |
| brawler-pound-blast | 172 | 23514 |
| king-plunge-cancel | 157 | 24908 |
| king-gordo-bounce | 151 | 26144 |
| king-gordo-bat | 152 | 26902 |
| sword-counter-melee | 106 | 17167 |
| sword-counter-reflect | 112 | 17690 |
| shield-break-dizzy | 640 | 81889 |
| knockdown-tech | 144 | 25959 |
| knockdown-tech-roll | 144 | 25949 |
| knockdown-stand | 165 | 29663 |
| knockdown-attack | 165 | 29672 |
| knockdown-roll | 165 | 26299 |
| knockdown-auto | 165 | 29940 |
| ko-respawn-training | 1100 | 162790 |
| ko-respawn-stocks | 848 | 121364 |
| countdown-held-input | 281 | 38693 |
| fuzz-1 | 1500 | 289187 |
| fuzz-2 | 1500 | 282177 |
| fuzz-3 | 1500 | 290127 |
| cpu-brawler-brawler-level2 | 2500 | 490499 |
| cpu-brawler-swordfighter-level2 | 2500 | 496044 |
| cpu-brawler-king-level2 | 2500 | 507241 |
| cpu-swordfighter-brawler-level2 | 2500 | 497061 |
| cpu-swordfighter-swordfighter-level2 | 2500 | 516355 |
| cpu-swordfighter-king-level2 | 2500 | 512497 |
| cpu-king-brawler-level2 | 2500 | 499111 |
| cpu-king-swordfighter-level2 | 2500 | 517686 |
| cpu-king-king-level2 | 2500 | 506204 |
| cpu-brawler-king-level0 | 2500 | 505767 |
| cpu-brawler-king-level1 | 2500 | 521076 |
| human-vs-cpu | 700 | 129354 |

## Exact setup overrides

Applied once immediately after createWorld; player indices are zero based. All unlisted fields retain TS defaults.

- **brawler-ground**: `[{"player":1,"x":510}]`
- **brawler-aerial-movement**: `[{"player":1,"x":920}]`
- **brawler-specials**: `[{"player":1,"x":510}]`
- **brawler-throw-f**: `[{"player":1,"x":510}]`
- **brawler-throw-b**: `[{"player":1,"x":510}]`
- **brawler-throw-u**: `[{"player":1,"x":510}]`
- **brawler-throw-d**: `[{"player":1,"x":510}]`
- **brawler-shield-hit**: `[{"player":1,"x":510}]`
- **swordfighter-ground**: `[{"player":1,"x":510}]`
- **swordfighter-aerial-movement**: `[{"player":1,"x":920}]`
- **swordfighter-specials**: `[{"player":1,"x":510}]`
- **swordfighter-throw-f**: `[{"player":1,"x":510}]`
- **swordfighter-throw-b**: `[{"player":1,"x":510}]`
- **swordfighter-throw-u**: `[{"player":1,"x":510}]`
- **swordfighter-throw-d**: `[{"player":1,"x":510}]`
- **swordfighter-shield-hit**: `[{"player":1,"x":510}]`
- **king-ground**: `[{"player":1,"x":510}]`
- **king-aerial-movement**: `[{"player":1,"x":920}]`
- **king-specials**: `[{"player":1,"x":510}]`
- **king-throw-f**: `[{"player":1,"x":510}]`
- **king-throw-b**: `[{"player":1,"x":510}]`
- **king-throw-u**: `[{"player":1,"x":510}]`
- **king-throw-d**: `[{"player":1,"x":510}]`
- **king-shield-hit**: `[{"player":1,"x":510}]`
- **ledge-climb**: `[{"player":0,"x":1010,"y":500,"ground":false,"surf":null,"state":"air","vy":2},{"player":1,"x":920}]`
- **ledge-attack**: `[{"player":0,"x":1010,"y":500,"ground":false,"surf":null,"state":"air","vy":2},{"player":1,"x":920}]`
- **ledge-roll**: `[{"player":0,"x":1010,"y":500,"ground":false,"surf":null,"state":"air","vy":2},{"player":1,"x":920}]`
- **ledge-jump**: `[{"player":0,"x":1010,"y":500,"ground":false,"surf":null,"state":"air","vy":2},{"player":1,"x":920}]`
- **ledge-drop**: `[{"player":0,"x":1010,"y":500,"ground":false,"surf":null,"state":"air","vy":2},{"player":1,"x":920}]`
- **walk-off-ledge**: `[{"player":0,"x":950},{"player":1,"x":920}]`
- **brawler-uspec-ledge**: `[{"player":0,"x":1010,"y":650,"ground":false,"surf":null,"state":"air","face":-1},{"player":1,"x":920}]`
- **brawler-pound-air**: `[{"player":1,"x":920}]`
- **brawler-pound-blast**: `[{"player":0,"x":1200,"y":-300,"ground":false,"surf":null,"state":"air"},{"player":1,"x":920}]`
- **king-plunge-cancel**: `[{"player":1,"x":920}]`
- **king-gordo-bounce**: `[{"player":0,"x":340},{"player":1,"x":950}]`
- **king-gordo-bat**: `[{"player":0,"x":340},{"player":1,"x":620}]`
- **sword-counter-melee**: `[{"player":1,"x":510}]`
- **sword-counter-reflect**: `[{"player":1,"x":650}]`
- **shield-break-dizzy**: `[{"player":1,"x":920}]`
- **knockdown-tech**: `[{"player":0,"dmg":120,"x":640},{"player":1,"x":640,"y":330,"ground":false,"surf":null,"state":"air"}]`
- **knockdown-tech-roll**: `[{"player":0,"dmg":120,"x":640},{"player":1,"x":640,"y":330,"ground":false,"surf":null,"state":"air"}]`
- **knockdown-stand**: `[{"player":0,"dmg":120,"x":640},{"player":1,"x":640,"y":330,"ground":false,"surf":null,"state":"air"}]`
- **knockdown-attack**: `[{"player":0,"dmg":120,"x":640},{"player":1,"x":640,"y":330,"ground":false,"surf":null,"state":"air"}]`
- **knockdown-roll**: `[{"player":0,"dmg":120,"x":640},{"player":1,"x":640,"y":330,"ground":false,"surf":null,"state":"air"}]`
- **knockdown-auto**: `[{"player":0,"dmg":120,"x":640},{"player":1,"x":640,"y":330,"ground":false,"surf":null,"state":"air"}]`
