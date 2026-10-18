package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.Visuals;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.hud.DebugHud;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

import java.util.List;

/** Custom F3: marks the text columns so DrawContextMixin restyles them. */
@Mixin(DebugHud.class)
public class DebugHudMixin {
	@Inject(method = "drawText", at = @At("HEAD"))
	private void mangoconfig$enter(DrawContext context, List<String> text, boolean left, CallbackInfo ci) {
		Visuals.scope = Visuals.Scope.F3;
	}

	@Inject(method = "drawText", at = @At("RETURN"))
	private void mangoconfig$leave(DrawContext context, List<String> text, boolean left, CallbackInfo ci) {
		Visuals.scope = Visuals.Scope.NONE;
	}
}
