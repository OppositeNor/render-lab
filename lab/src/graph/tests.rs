use super::*;
use crate::investigation::{ShadingBuffer, ShadingPass, ShadingTarget};

/// A chain: the pre-pass, `early`, the scene pass, then `after`.
fn shading(early: &'static [ShadingPass], after: &'static [ShadingPass]) -> &'static SceneShading {
    use crate::investigation::FrameNode;
    static TARGETS: &[ShadingTarget] = &[
        ShadingTarget {
            name: "color",
            format: wgpu::TextureFormat::Rgba16Float,
            divisor: 1,
        },
        ShadingTarget {
            name: "blur",
            format: wgpu::TextureFormat::Rgba16Float,
            divisor: 1,
        },
        ShadingTarget {
            name: "unused",
            format: wgpu::TextureFormat::Rgba16Float,
            divisor: 1,
        },
    ];
    static BUFFERS: &[ShadingBuffer] = &[ShadingBuffer {
        name: "tiles",
        words_per_tile: 4,
        tile: 16,
    }];
    let passes: &'static [ShadingPass] = Box::leak(
        early
            .iter()
            .chain(after)
            .map(|p| ShadingPass { ..*p })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );
    let mut nodes = vec![FrameNode::Prepass];
    nodes.extend((0..early.len()).map(FrameNode::Compute));
    nodes.push(FrameNode::Scene);
    nodes.extend((early.len()..passes.len()).map(FrameNode::Compute));
    Box::leak(Box::new(SceneShading {
        module: "test",
        fragment: "sceneFS",
        data: Vec::new,
        verify: &[],
        targets: TARGETS,
        outputs: &["color"],
        passes,
        prepass: Some("prepassFS"),
        prepass_outputs: &[],
        buffers: BUFFERS,
        scene_reads: &["tiles"],
        depth_views: None,
        volumes: &[],
        update: None,
        resolution: None,
        history: &[],
        shows: &[],
        program: None,
        nodes: Box::leak(nodes.into_boxed_slice()),
    }))
}

#[test]
fn a_program_places_its_passes_where_its_nodes_are() {
    use crate::investigation::{
        Access, FrameNode, ImageKind, PassKind, ProgramBuffer, ProgramImage, ProgramPass,
        ProgramUse, ShadingProgram, Which,
    };
    static IMAGES: &[ProgramImage] = &[ProgramImage {
        name: "grid",
        kind: ImageKind::D3,
        size: [8, 8, 8],
        format: wgpu::TextureFormat::R32Uint,
        count: 2,
        mips: 1,
        viewport: 0,
    }];
    static BUFFERS: &[ProgramBuffer] = &[ProgramBuffer {
        name: "list",
        bytes: 64,
        indirect: true,
        count: 1,
    }];
    static PASSES: &[ProgramPass] = &[
        ProgramPass {
            name: "buildCS",
            kind: PassKind::Compute {
                entry: "buildCS",
                workgroup: [4, 4, 4],
            },
            uses: &[
                ProgramUse {
                    binding: 1,
                    resource: "grid",
                    access: Access::Write,
                    which: Which::Instance,
                },
                ProgramUse {
                    binding: 2,
                    resource: "list",
                    access: Access::Atomic,
                    which: Which::First,
                },
            ],
            instances: 2,
        },
        ProgramPass {
            name: "gatherCS",
            kind: PassKind::Compute {
                entry: "gatherCS",
                workgroup: [8, 8, 1],
            },
            uses: &[ProgramUse {
                binding: 1,
                resource: "grid",
                access: Access::Load,
                which: Which::All,
            }],
            instances: 1,
        },
        // This frame's colour, once the scene pass has written it.
        ProgramPass {
            name: "keepCS",
            kind: PassKind::Compute {
                entry: "keepCS",
                workgroup: [8, 8, 1],
            },
            uses: &[
                ProgramUse {
                    binding: 1,
                    resource: "hdr",
                    access: Access::Load,
                    which: Which::First,
                },
                ProgramUse {
                    binding: 2,
                    resource: "grid",
                    access: Access::Write,
                    which: Which::Mip(0),
                },
            ],
            instances: 1,
        },
    ];
    static PROGRAM: ShadingProgram = ShadingProgram {
        images: IMAGES,
        buffers: BUFFERS,
        passes: PASSES,
        scene_uses: &[],
        init: || Box::new(()),
        plan: |_, _| Vec::new(),
        warmup: 0,
        record_history: false,
    };
    let s = shading(&[], &[]);
    // No chain (the scene pass writes the frame), as a program's investigation may have.
    // Its pre-pass does not run without a chain.
    static NODES: &[FrameNode] = &[
        FrameNode::Program(0),
        FrameNode::Prepass,
        FrameNode::Program(1),
        FrameNode::Scene,
        FrameNode::Program(2),
    ];
    let with = |nodes: &'static [FrameNode]| -> &'static SceneShading {
        Box::leak(Box::new(SceneShading {
            program: Some(&PROGRAM),
            outputs: &[],
            scene_reads: &[],
            nodes,
            ..*s
        }))
    };
    let g = FrameGraph::new(Some(with(NODES)), &[]);
    assert!(g.errors.is_empty(), "{:?}", g.errors);
    // Its commands are recorded in three runs: before the pre-pass, between it and the scene pass, after the scene pass.
    assert_eq!(with(NODES).program_runs(), vec![0, 1, 2]);
    // A pass in no node, or in two, is an error; so is a pre-pass after the scene pass.
    let missing = FrameGraph::new(
        Some(with(&[
            FrameNode::Prepass,
            FrameNode::Program(0),
            FrameNode::Program(1),
            FrameNode::Scene,
        ])),
        &[],
    );
    assert!(
        missing
            .errors
            .iter()
            .any(|e| e.contains("program pass `keepCS` is in no node")),
        "{:?}",
        missing.errors
    );
    let twice = FrameGraph::new(
        Some(with(&[
            FrameNode::Program(0),
            FrameNode::Program(1),
            FrameNode::Scene,
            FrameNode::Program(2),
            FrameNode::Program(2),
            FrameNode::Prepass,
        ])),
        &[],
    );
    assert!(
        twice
            .errors
            .iter()
            .any(|e| e.contains("`keepCS` is in 2 nodes")),
        "{:?}",
        twice.errors
    );
    assert!(
        twice
            .errors
            .iter()
            .any(|e| e.contains("pre-pass comes after the scene pass")),
        "{:?}",
        twice.errors
    );
    let names: Vec<&str> = g.order.iter().map(|&i| g.nodes[i].name).collect();
    assert_eq!(
        names,
        [
            "buildCS",
            "gatherCS",
            "sceneFS",
            "keepCS",
            "overlay",
            "composite"
        ]
    );
    // It reads `hdr` after the scene pass writes it.
    let keep = g.nodes.iter().position(|n| n.name == "keepCS").unwrap();
    let scene = g.nodes.iter().position(|n| n.name == "sceneFS").unwrap();
    assert!(g.edges.iter().any(|e| (e.0, e.1) == (scene, keep)));
    // A program's passes write what outlives the frame: they run though nothing reads their writes this frame.
    assert!(g.live.iter().all(|l| *l));
    let build = &g.nodes[g.node("buildCS").unwrap()];
    assert_eq!(
        (build.reads.clone(), build.writes.clone()),
        (vec!["list"], vec!["grid", "list"])
    );
    let grid = &g.resources[g.resource("grid").unwrap()];
    assert_eq!(grid.lifetime, Lifetime::Persistent);
    assert_eq!(grid.shape.describe(), "R32Uint, 8x8x8, 2 of them");
    assert_eq!(inspect_entry(&grid.shape).0, "inspectVolumeUintCS");
    assert_eq!(default_show(&grid.shape), Show::Hash);
}

#[test]
fn order_follows_accesses_and_keeps_declaration_order() {
    let s = shading(
        &[ShadingPass {
            entry: "tilesCS",
            reads: &["depth"],
            writes: &["tiles"],
            workgroup: [16, 16],
        }],
        &[
            ShadingPass {
                entry: "blurCS",
                reads: &["color"],
                writes: &["blur"],
                workgroup: [8, 8],
            },
            ShadingPass {
                entry: "mergeCS",
                reads: &["blur", "depth"],
                writes: &["hdr"],
                workgroup: [8, 8],
            },
        ],
    );
    let g = FrameGraph::new(Some(s), &[]);
    assert!(g.errors.is_empty(), "{:?}", g.errors);
    let names: Vec<&str> = g.order.iter().map(|&i| g.nodes[i].name).collect();
    assert_eq!(
        names,
        [
            "prepassFS",
            "tilesCS",
            "sceneFS",
            "blurCS",
            "mergeCS",
            "overlay",
            "composite"
        ]
    );
    assert!(g.live.iter().all(|l| *l));
    // The scene pass reads the pre-pass's depth and the tiles.
    let scene = g.node("sceneFS").unwrap();
    assert!(g.edges.contains(&(
        g.node("tilesCS").unwrap(),
        scene,
        Edge::Data(g.resource("tiles").unwrap())
    )));
    assert!(g.inspected("blurCS.blur").is_ok());
    assert!(g.inspected("blurCS.hdr").is_err());
    assert_eq!(g.result_of(g.node("composite").unwrap()), Some("hdr"));
    assert_eq!(g.result_of(g.node("blurCS").unwrap()), Some("blur"));
    assert_eq!(g.last_user("blur"), g.node("blurCS"));
    // What each node is compiled from.
    assert_eq!(
        source_of(Some(s), &g.nodes[g.node("blurCS").unwrap()]),
        (s.module, vec!["blurCS"])
    );
    assert_eq!(
        source_of(Some(s), &g.nodes[g.node("sceneFS").unwrap()]),
        (s.module, vec!["sceneVS", "sceneFS"])
    );
    assert_eq!(
        source_of(Some(s), &g.nodes[g.node("composite").unwrap()]).0,
        crate::shaders::SCENE
    );
    // The frame graph as a 2D scene: its sections' boxes and the edges between them.
    let section = |id: &str, passes: &[&str]| Section {
        id: id.into(),
        title: id.into(),
        passes: passes.iter().map(|p| p.to_string()).collect(),
        ..Default::default()
    };
    let scene = GraphScene {
        sections: vec![
            section("DEPTH", &["prepassFS", "tilesCS"]),
            section("SCENE", &["sceneFS"]),
            section("POST", &["blurCS", "mergeCS"]),
        ],
        omit: Vec::new(),
    };
    let d = g.scene_graph(&scene).unwrap();
    assert_eq!(
        d.nodes.iter().map(|n| n.rows.len()).collect::<Vec<_>>(),
        [2, 1, 2]
    );
    assert_eq!(d.nodes[0].rows[1].label, "tiles");
    // DEPTH reaches POST through SCENE (the blur reads the scene's colour): no edge of its own.
    assert_eq!(
        d.edges.iter().map(|e| (e.from, e.to)).collect::<Vec<_>>(),
        [(0, 1), (1, 2)]
    );
    assert_eq!(d.named("tilesCS"), Some(0));
    let parsed = GraphScene::parse("[[section]]\nid = \"DEPTH\"\ntitle-text = \"t\"\npasses = [\"prepassFS\"]\npicture = \"prepassFS.depth\"\n", "test", &|k| (k == "t").then(|| "Depth".to_string())).unwrap();
    assert_eq!(
        (
            parsed.sections[0].title.as_str(),
            parsed.sections[0].picture.as_deref()
        ),
        ("Depth", Some("prepassFS.depth"))
    );
    assert!(
        GraphScene::parse(
            "[[section]]\nid = \"A\"\ntitle-text = \"missing\"\n",
            "test",
            &|_| None
        )
        .is_err()
    );
    let short = GraphScene {
        sections: vec![section("DEPTH", &["prepassFS"])],
        omit: Vec::new(),
    };
    assert!(
        g.scene_graph(&short)
            .unwrap_err()
            .contains("passes in no section: tilesCS")
    );
    assert_eq!(g.last_user("hdr"), g.node("mergeCS"));
    // A resource's view: the investigation's, else its format's.
    let shown = FrameGraph::new(
        Some(Box::leak(Box::new(SceneShading {
            shows: &[("tiles", "count?word=3&range=0,8")],
            ..*s
        }))),
        &[],
    );
    let (show, params) = shown.view_of(shown.resource("tiles").unwrap());
    assert_eq!(
        (show, params["word"].clone(), params["range"].clone()),
        (Show::Count, vec![3.0], vec![0.0, 8.0])
    );
    assert_eq!(
        shown.view_of(shown.resource("depth").unwrap()).0,
        Show::Depth
    );
    let bad = FrameGraph::new(
        Some(Box::leak(Box::new(SceneShading {
            shows: &[("tiles", "count?wurd=3"), ("nothing", "hash")],
            ..*s
        }))),
        &[],
    );
    assert_eq!(bad.errors.len(), 2, "{:?}", bad.errors);
}

#[test]
fn reads_before_writes_and_culling() {
    let s = shading(
        &[],
        &[
            ShadingPass {
                entry: "mergeCS",
                reads: &["blur"],
                writes: &["hdr"],
                workgroup: [8, 8],
            },
            ShadingPass {
                entry: "blurCS",
                reads: &["color"],
                writes: &["blur"],
                workgroup: [8, 8],
            },
            ShadingPass {
                entry: "debugCS",
                reads: &["color"],
                writes: &["unused"],
                workgroup: [8, 8],
            },
        ],
    );
    let g = FrameGraph::new(Some(s), &[]);
    assert!(
        g.errors
            .iter()
            .any(|e| e.contains("mergeCS reads `blur` before")),
        "{:?}",
        g.errors
    );
    assert!(!g.live[g.node("debugCS").unwrap()]);
    assert_eq!(g.unread(), vec![("blurCS", "blur"), ("debugCS", "unused")]);
    // An inspected resource keeps its writer alive.
    let g = FrameGraph::new(Some(s), &["unused"]);
    assert!(g.live[g.node("debugCS").unwrap()]);
}

#[test]
fn history_and_repeated_entries() {
    let s = shading(
        &[ShadingPass {
            entry: "tilesCS",
            reads: &["depth"],
            writes: &["tiles"],
            workgroup: [16, 16],
        }],
        &[
            ShadingPass {
                entry: "mergeCS",
                reads: &["blur", "color"],
                writes: &["hdr"],
                workgroup: [8, 8],
            },
            ShadingPass {
                entry: "blurCS",
                reads: &["color"],
                writes: &["blur"],
                workgroup: [8, 8],
            },
            ShadingPass {
                entry: "blurCS",
                reads: &["color"],
                writes: &["blur"],
                workgroup: [8, 8],
            },
        ],
    );
    let g = FrameGraph::new(
        Some(Box::leak(Box::new(SceneShading {
            history: &["blur"],
            ..*s
        }))),
        &[],
    );
    assert!(g.errors.is_empty(), "{:?}", g.errors);
    assert_eq!(
        g.resources[g.resource("blur").unwrap()].lifetime,
        Lifetime::History
    );
    // The next frame reads what blurCS writes.
    assert!(g.live[g.node("blurCS").unwrap()]);
    assert!(g.node("blurCS#2").is_some());
}

#[test]
fn without_a_chain() {
    let g = FrameGraph::new(None, &[]);
    assert!(g.errors.is_empty());
    let names: Vec<&str> = g.order.iter().map(|&i| g.nodes[i].name).collect();
    assert_eq!(names, ["scene", "overlay", "composite"]);
}
