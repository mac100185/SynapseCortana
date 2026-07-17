"""Prepara el .glb del usuario (Tripo3D) para SynapseCortana:
1. Decima de 194k a ~15-20k triángulos.
2. Reemplaza el material PBR por el holográfico azul.
3. Re-escala a 1.75m de altura.
4. Exporta como cortana_completa.glb.
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

# ===== 2. IMPORTAR EL .GLB DEL USUARIO =====
glb_path = "/home/macarthur/Proyectos/SynapseCortana/frontend/assets/Cortana.glb"
bpy.ops.import_scene.gltf(filepath=glb_path)

# Encontrar el mesh importado.
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

    # ===== 3. RE-ESCALAR A 1.75m DE ALTURA =====
    current_height = obj.dimensions.z
    if current_height > 0:
        scale_factor = 1.75 / current_height
        obj.scale = (scale_factor, scale_factor, scale_factor)
        bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
        print("Re-escalado a 1.75m (factor={})".format(round(scale_factor, 4)))
        print(
            "Nuevas dimensiones: {}".format(tuple(round(d, 3) for d in obj.dimensions))
        )

    # ===== 4. DECIMAR A ~15-20K TRIANGULOS =====
    # El modificador Decimate con ratio = 15000 / 194006 ≈ 0.077.
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

    # Aplicar el modificador.
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.modifier_apply(modifier="Decimate")

    final_verts = len(obj.data.vertices)
    final_tris = sum(len(p.vertices) - 2 for p in obj.data.polygons)
    print("Despues de decimate: {} verts, ~{} tris".format(final_verts, final_tris))

    # ===== 5. CREAR MATERIAL HOLOGRAFICO AZUL =====
    mat = bpy.data.materials.new(name="CortanaHolografico")
    mat.use_nodes = True
    mat.blend_method = "BLEND"
    mat.shadow_method = "NONE"
    nodes = mat.node_tree.nodes
    links = mat.node_tree.links
    for n in list(nodes):
        nodes.remove(n)

    output = nodes.new("ShaderNodeOutputMaterial")
    output.location = (600, 0)

    bsdf = nodes.new("ShaderNodeBsdfPrincipled")
    bsdf.location = (200, 100)
    bsdf.inputs["Base Color"].default_value = (0.118, 0.565, 0.9, 1.0)  # #1E90FF
    bsdf.inputs["Roughness"].default_value = 0.3
    bsdf.inputs["Transmission Weight"].default_value = 0.6
    bsdf.inputs["IOR"].default_value = 1.3
    bsdf.inputs["Alpha"].default_value = 0.7

    emission = nodes.new("ShaderNodeEmission")
    emission.location = (200, -100)
    emission.inputs["Color"].default_value = (0.118, 0.565, 0.9, 1.0)
    emission.inputs["Strength"].default_value = 3.0

    mix = nodes.new("ShaderNodeMixShader")
    mix.location = (400, 0)
    mix.inputs["Fac"].default_value = 0.5

    wave = nodes.new("ShaderNodeTexWave")
    wave.location = (-100, -200)
    wave.inputs["Scale"].default_value = 50.0
    wave.wave_type = "BANDS"

    ramp = nodes.new("ShaderNodeValToRGB")
    ramp.location = (100, -200)
    ramp.color_ramp.elements[0].position = 0.4
    ramp.color_ramp.elements[1].position = 0.6

    links.new(wave.outputs["Fac"], ramp.inputs["Fac"])
    links.new(ramp.outputs["Color"], mix.inputs["Fac"])
    links.new(bsdf.outputs["BSDF"], mix.inputs[1])
    links.new(emission.outputs["Emission"], mix.inputs[2])
    links.new(mix.outputs["Shader"], output.inputs["Surface"])

    mat.use_backface_culling = False
    mat.show_transparent_back = True

    # ===== 6. APLICAR MATERIAL HOLOGRAFICO =====
    obj.data.materials.clear()
    obj.data.materials.append(mat)
    obj.name = "Cortana_Holografica"
    print("Material holografico aplicado a {}".format(obj.name))

    # ===== 7. SMOOTH SHADING =====
    bpy.ops.object.shade_smooth()
    print("Smooth shading aplicado")

    # ===== 8. CAMARA Y LUCES PARA RENDER =====
    bpy.ops.object.camera_add(location=(0, -3.0, 1.0))
    cam = bpy.context.active_object
    cam.name = "Camara_Synapse"
    cam.rotation_euler = (math.radians(85), 0, 0)
    bpy.context.scene.camera = cam

    bpy.ops.object.light_add(type="SUN", location=(2, -2, 5))
    sun = bpy.context.active_object
    sun.name = "Luz_Sol"
    sun.data.energy = 3.0
    sun.rotation_euler = (math.radians(45), 0, math.radians(30))

    bpy.ops.object.light_add(type="AREA", location=(-2, -1, 2))
    fill = bpy.context.active_object
    fill.name = "Luz_Relleno_Azul"
    fill.data.energy = 50.0
    fill.data.color = (0.118, 0.565, 0.9)
    fill.data.size = 2.0

    # ===== 9. CONFIGURAR VIEWPORT =====
    for area in bpy.context.screen.areas:
        if area.type == "VIEW_3D":
            for space in area.spaces:
                if space.type == "VIEW_3D":
                    space.shading.type = "MATERIAL"
                    space.region_3d.view_location = (0, 0, 0.9)
                    space.region_3d.view_distance = 4.0
                    break
            break

    # ===== 10. EXPORTAR A GLB =====
    # Seleccionar SOLO el modelo.
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
    print("\n=== EXPORTACION COMPLETADA ===")
    print("  Archivo: {}".format(output_path))
    print("  Objeto: {}".format(obj.name))
    print("  Vertices: {}".format(final_verts))
    print("  Triangulos: ~{}".format(final_tris))
    print("  Altura: {}m".format(round(obj.dimensions.z, 3)))
    print("  Material: holografico azul #1E90FF")
