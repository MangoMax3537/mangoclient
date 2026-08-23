package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.Mods;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.hud.InGameHud;
import net.minecraft.client.render.RenderTickCounter;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * The parts of the vanilla HUD a player may want gone, and the crosshair they
 * may want replaced.
 *
 * 1.21-1.21.1 variant: no renderNauseaOverlay yet, so Hide Nausea is inert.
 *
 * Every hook does the same thing: if the setting is off, vanilla draws as it
 * always has and this class costs one field read.
 */
@Mixin(InGameHud.class)
public class HudOverlayMixin {
	@Inject(method = "renderCrosshair", at = @At("HEAD"), cancellable = true)
	private void mangoconfig$crosshair(DrawContext context, RenderTickCounter tickCounter, CallbackInfo ci) {
		if (!Mods.crosshair.value) return;
		// Vanilla's own first-person check sits inside the method this replaces,
		// so third person is this mod's to decide - and by default it stays
		// vanilla-like: no crosshair while the camera is behind you.
		if (Mods.crosshairHideThirdPerson.value
			&& !MinecraftClient.getInstance().options.getPerspective().isFirstPerson()) {
			ci.cancel();
			return;
		}
		Mods.renderCrosshair(context);
		ci.cancel();
	}

	// Two methods share this name; the one that takes the tick counter is the
	// one the HUD calls.
	@Inject(
		method = "renderScoreboardSidebar(Lnet/minecraft/client/gui/DrawContext;"
			+ "Lnet/minecraft/client/render/RenderTickCounter;)V",
		at = @At("HEAD"), cancellable = true)
	private void mangoconfig$scoreboard(DrawContext context, RenderTickCounter tickCounter, CallbackInfo ci) {
		if (Mods.hideScoreboard.value) ci.cancel();
	}

	@Inject(method = "renderStatusEffectOverlay", at = @At("HEAD"), cancellable = true)
	private void mangoconfig$effectIcons(DrawContext context, RenderTickCounter tickCounter, CallbackInfo ci) {
		if (Mods.hideEffectIcons.value) ci.cancel();
	}

	@Inject(method = "renderPortalOverlay", at = @At("HEAD"), cancellable = true)
	private void mangoconfig$portalOverlay(DrawContext context, float nauseaStrength, CallbackInfo ci) {
		if (Mods.hidePortalOverlay.value) ci.cancel();
	}

	@Inject(method = "renderHeldItemTooltip", at = @At("HEAD"), cancellable = true)
	private void mangoconfig$itemName(DrawContext context, CallbackInfo ci) {
		if (Mods.hideItemName.value) ci.cancel();
	}
}
