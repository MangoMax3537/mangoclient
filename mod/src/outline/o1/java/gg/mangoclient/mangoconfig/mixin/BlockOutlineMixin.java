package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.Visuals;
import net.minecraft.client.render.VertexConsumer;
import net.minecraft.client.render.WorldRenderer;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.util.shape.VoxelShape;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

/**
 * Block Overlay, 1.20.5-1.21.1: the outline's colour is four floats written
 * straight into the one call that draws it, so that call is redirected.
 */
@Mixin(WorldRenderer.class)
public abstract class BlockOutlineMixin {
	@Shadow
	private static void drawCuboidShapeOutline(MatrixStack matrices, VertexConsumer lines, VoxelShape shape,
			double x, double y, double z, float r, float g, float b, float a) {
	}

	@Redirect(method = "drawBlockOutline", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/render/"
		+ "WorldRenderer;drawCuboidShapeOutline(Lnet/minecraft/client/util/math/MatrixStack;"
		+ "Lnet/minecraft/client/render/VertexConsumer;Lnet/minecraft/util/shape/VoxelShape;DDDFFFF)V"))
	private void mangoconfig$outline(MatrixStack matrices, VertexConsumer lines, VoxelShape shape,
			double x, double y, double z, float r, float g, float b, float a) {
		int vanilla = (Math.round(a * 255) << 24) | (Math.round(r * 255) << 16) | (Math.round(g * 255) << 8)
			| Math.round(b * 255);
		int c = Visuals.outline(vanilla);
		drawCuboidShapeOutline(matrices, lines, shape, x, y, z,
			((c >> 16) & 0xFF) / 255f, ((c >> 8) & 0xFF) / 255f, (c & 0xFF) / 255f, ((c >>> 24) & 0xFF) / 255f);
	}
}
