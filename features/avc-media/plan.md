# avc-media — Implementation plan

## Confirm the relinked composition in Resolume
`glitch dynamics v3.1.1 cv control.avc` is written and verified at the file level: 78 of
8694 lines differ and every one is a path or the composition name, CRLF preserved, XML
parses, all 15 targets exist. What is not yet confirmed is Resolume's own view of it —
open it and check that every clip loads its HAP file with no red/missing clips, and that
playback is smoother than the ProRes/H.264 original.

## Confirm playback
The claim the whole feature rests on. A/B a converted clip against its original.
