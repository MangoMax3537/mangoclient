package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.Visuals;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.hud.InGameHud;
import net.minecraft.scoreboard.ScoreboardObjective;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Marks the scoreboard while it draws, so DrawContextMixin can make its
 * background see-through. Vanilla fills it from inside a lambda in some
 * versions; a scope around the whole method catches those too.
 */
@Mixin(InGameHud.class)
public class InGameHudVisualsMixin {
	@Inject(method = "renderScoreboardSidebar(Lnet/minecraft/client/gui/DrawContext;"
		+ "Lnet/minecraft/scoreboard/ScoreboardObjective;)V", at = @At("HEAD"))
	private void mangoconfig$scoreboardEnter(DrawContext context, ScoreboardObjective objective, CallbackInfo ci) {
		Visuals.scope = Visuals.Scope.SCOREBOARD;
	}

	@Inject(method = "renderScoreboardSidebar(Lnet/minecraft/client/gui/DrawContext;"
		+ "Lnet/minecraft/scoreboard/ScoreboardObjective;)V", at = @At("RETURN"))
	private void mangoconfig$scoreboardLeave(DrawContext context, ScoreboardObjective objective, CallbackInfo ci) {
		Visuals.scope = Visuals.Scope.NONE;
	}
}
