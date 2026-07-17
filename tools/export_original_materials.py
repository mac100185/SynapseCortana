"""Prepara el .glb del usuario (Tripo3D) conservando los materiales originales.
NO aplica el material holográfico azul. Solo decima y re-escala.
"""

import math

import bpy

# ===== 1. LIMPIAR ESCENA =====
bpy.ops.object.select_all(action="SELECT")
bpy.ops.object.delete()
for block in list(bpy.data.meshes):
    if block.users == 0:
        bpy.data.meshes.remove(block)
for block in list(bpy.data.materials):
    if block.users == 0:
        bpy.data.materials.remove(block)
for block in list(bpy.data.cameras):
    if block.users == 0:
        bpy.data.cameras.remove(block)
for block in list(bpy.data.lights):
    if block.users == 0:
        bpy.data.lights.remove(block)
print("Escena limpia")

# ===== 2. IMPORTAR EL .GLB ORIGINAL DEL USUARIO =====
glb_path = "/home/macarthur/Proyectos/SynapseCortana/frontend/assets/Cortana.glb"
bpy.ops.import_scene.gltf(filepath=glb_path)

meshes = [o for o in bpy.data.objects if o.type == "MESH"]
if not meshes:
    print("ERROR: no se importo ninguna malla")
else:
    obj = meshes[0]
    original_name = obj.name
    original_verts = len(obj.data.vertices)
    original_tris = sum(len(p.vertices) - 2 for p in obj.data.polygons)
    print(
        "Modelo importado: {} ({} verts, ~{} tris)".format(
            original_name, original_verts, original_tris
        )
    )
    print(
        "Dimensiones originales: {}".format(tuple(round(d, 3) for d in obj.dimensions))
    )

    # Listar materiales originales.
    print("Materiales originales:")
    for mat in obj.data.materials:
        if mat:
            print(
                "  - {} (blend={}, nodes={})".format(
                    mat.name, mat.blend_method, mat.use_nodes
                )
            )

    # ===== 3. RE-ESCALAR A 1.75m DE ALTURA =====
    current_height = obj.dimensions.z
    if current_height > 0:
        scale_factor = 1.75 / current_height
        obj.scale = (scale_factor, scale_factor, scale_factor)
        bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
        print("Re-escalado a 1.75m (factor={})".format(round(scale_factor, 4)))

    # ===== 4. DECIMAR A ~18K TRIANGULOS =====
    target_tris = 18000
    decimate_ratio = min(1.0, target_tris / max(original_tris, 1))
    print(
        "Decimate ratio: {} (objetivo: {} tris)".format(
            round(decimate_ratio, 4), target_tris
        )
    )

    decimate = obj.modifiers.new(name="Decimate", type="DECIMATE")
    decimate.decimate_type = "COLLAPSE"
    decimate.ratio = decimate_ratio
    decimate.use_collapse_triangulate = True

    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.modifier_apply(modifier="Decimate")

    final_verts = len(obj.data.vertices)
    final_tris = sum(len(p.vertices) - 2 for p in obj.data.polygons)
    print("Despues de decimate: {} verts, ~{} tris".format(final_verts, final_tris))

    # ===== 5. SMOOTH SHADING =====
    bpy.ops.object.shade_smooth()
    print("Smooth shading aplicado")

    # ===== 6. EXPORTAR CON MATERIALES ORIGINALES =====
    obj.name = "Cortana"
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj

    output_path = (
        "/home/macarthur/Proyectos/SynapseCortana/frontend/assets/cortana_completa.glb"
    )
    bpy.ops.export_scene.gltf(
        filepath=output_path,
        export_format="GLB",
        export_apply=True,
        use_selection=True,
    )
    print("\n=== EXPORTACION COMPLETADA (materiales originales) ===")
    print("  Archivo: {}".format(output_path))
    print("  Objeto: {}".format(obj.name))
    print("  Vertices: {}".format(final_verts))
    print("  Triangulos: ~{}".format(final_tris))
    print("  Materiales: conservados del .glb original")
