# Pinned Unicode17 inputs

The files in `17.0.0/` are vendored Unicode data used to generate pure Sengoo
property decision trees and run the official extended-grapheme corpus. They
remain under the accompanying Unicode `LICENSE.txt`. Compiler builds never
download these inputs. `scripts/generate-unicode-sg.py` verifies SHA256 before
reading them and supports an offline `--check` comparison of generated output.

| File | Official source | SHA256 |
| --- | --- | --- |
| GraphemeBreakProperty.txt | https://www.unicode.org/Public/17.0.0/ucd/auxiliary/GraphemeBreakProperty.txt | d6b51d1d2ae5c33b451b7ed994b48f1f4dc62b2272a5831e7fd418514a6bae89 |
| GraphemeBreakTest.txt | https://www.unicode.org/Public/17.0.0/ucd/auxiliary/GraphemeBreakTest.txt | e2d134d2c52919bace503ebb6a551c1855fe1a1faec18478c78fff254a1793ec |
| DerivedCoreProperties.txt | https://www.unicode.org/Public/17.0.0/ucd/DerivedCoreProperties.txt | 24c7fed1195c482faaefd5c1e7eb821c5ee1fb6de07ecdbaa64b56a99da22c08 |
| emoji-data.txt | https://www.unicode.org/Public/17.0.0/ucd/emoji/emoji-data.txt | 2cb2bb9455cda83e8481541ecf5b6dfda66a3bb89efa3fa7c5297eccf607b72b |
| EastAsianWidth.txt | https://www.unicode.org/Public/17.0.0/ucd/EastAsianWidth.txt | ea7ce50f3444a050333448dffef1cadd9325af55cbb764b4a2280faf52170a33 |
| LICENSE.txt | https://www.unicode.org/license.txt | e7a93b009565cfce55919a381437ac4db883e9da2126fa28b91d12732bc53d96 |

Downloaded on 2026-11-04. Data tables implement Grapheme_Cluster_Break,
Indic_Conjunct_Break, Extended_Pictographic, Emoji_Presentation,
Default_Ignorable_Code_Point and W/F East_Asian_Width. Hangul LV/LVT is computed
algorithmically. Terminal width remains an explicitly documented policy and is
not a normative Unicode promise about all fonts or terminal emulators.
