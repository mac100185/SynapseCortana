"""Procesa Cortana2.glb: decima, re-escala, conserva materiales originales."""

import bpy

# Limpiar escena.
bpy.ops.object.select_all(action="SELECT")
bpy.ops.object.delete()
for block in list(bpy.data.meshes):
    if block.users == 0:
        bpy.data.meshes.remove(block)
for block in list(bpy.data.materials):
    if block.users == 0:
        bpy.data.materials.remove(block)

# Importar .glb.
glb_path = "/home/macarthur/Proyectos/SynapseCortana/frontend/assets/Cortana2.glb"
bpy.ops.import_scene.gltf(filepath=glb_path)

meshes = [o for o in bpy.data.objects if o.type == "MESH"]
if not meshes:
    print("ERROR: no se importo ninguna malla")
else:
    obj = meshes[0]
    original_verts = len(obj.data.vertices)
    original_tris = sum(len(p.vertices) - 2 for p in obj.data.polygons)
    print(
        "Modelo: {} ({} verts, ~{} tris)".format(
            obj.name, original_verts, original_tris
        )
    )
    print("Dimensiones: {}".format(tuple(round(d, 3) for d in obj.dimensions)))

    # Re-escalar a 1.75m.
    h = obj.dimensions.z
    if h > 0:
        f = 1.75 / h
        obj.scale = (f, f, f)
        bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
        print("Re-escalado a 1.75m")

    # Decimar.
    target = 18000
    ratio = min(1.0, target / max(original_tris, 1))
    dec = obj.modifiers.new(name="Decimate", type="DECIMATE")
    dec.decimate_type = "COLLAPSE"
    dec.ratio = ratio
    dec.use_collapse_triangulate = True
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.modifier_apply(modifier="Decimate")
    print(
        "Decimado: {} verts, ~{} tris".format(
            len(obj.data.vertices), sum(len(p.vertices) - 2 for p in obj.data.polygons)
        )
    )

    # Smooth.
    bpy.ops.object.shade_smooth()

    # Exportar.
    obj.name = "Cortana"
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj

    out = (
        "/home/macarthur/Proyectos/SynapseCortana/frontend/assets/cortana_completa.glb"
    )
    bpy.ops.export_scene.gltf(
        filepath=out, export_format="GLB", export_apply=True, use_selection=True
    )
    print("Exportado: {}".format(out))
