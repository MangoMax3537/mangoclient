package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.Visuals;
import net.minecraft.client.render.VertexConsumer;
import net.minecraft.client.render.VertexRendering;
import net.minecraft.client.render.entity.EntityRenderDispatcher;
import net.minecraft.client.util.math.MatrixStack;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyArg;
import org.spongepowered.asm.mixin.injection.Redirect;

/**
 * Hitboxes, 1.21.5-1.21.8: the box and the eye line are EntityHitbox records,
 * each drawn as one coloured box; the look vector is drawn after them.
 */
@Mixin(EntityRenderDispatcher.class)
public class HitboxMixin {
	@Redirect(method = "renderHitbox(Lnet/minecraft/client/util/math/MatrixStack;"
		+ "Lnet/minecraft/client/render/VertexConsumer;Lnet/minecraft/client/render/entity/state/EntityHitbox;)V",
		at = @At(value = "INVOKE", target = "Lnet/minecraft/client/render/VertexRendering;"
			+ "drawBox(Lnet/minecraft/client/util/math/MatrixStack;Lnet/minecraft/client/render/VertexConsumer;DDDDDDFFFF)V"))
	private static void mangoconfig$box(MatrixStack matrices, VertexConsumer lines, double x1, double y1, double z1,
			double x2, double y2, double z2, float r, float g, float b, float a) {
		float[] c = Visuals.hitbox(r, g, b, a);
		VertexRendering.drawBox(matrices, lines, x1, y1, z1, x2, y2, z2, c[0], c[1], c[2], c[3]);
	}

	@ModifyArg(method = "renderHitboxes(Lnet/minecraft/client/util/math/MatrixStack;"
		+ "Lnet/minecraft/client/render/entity/state/EntityHitboxAndView;Lnet/minecraft/client/render/VertexConsumer;F)V",
		at = @At(value = "INVOKE", target = "Lnet/minecraft/client/render/VertexRendering;drawVector("
			+ "Lnet/minecraft/client/util/math/MatrixStack;Lnet/minecraft/client/render/VertexConsumer;"
			+ "Lorg/joml/Vector3f;Lnet/minecraft/util/math/Vec3d;I)V"),
		index = 4)
	private static int mangoconfig$look(int colour) {
		return Visuals.hitbox(colour);
	}
}
