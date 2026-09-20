/**
 * The `::emote` / `!emote` chat shortcut (cache CS2 script 7304).
 *
 * That script matches its keyword list by *prefix*, so it claims anything
 * starting with an emote name: `::runes` triggers the Run emote and script 73
 * then wipes the chat buffer, so the command never reaches the server. That is
 * how the stock client behaves (`::runescape` emotes in OSRS too), but this
 * server owns the `::` namespace, so the VM only lets script 7304 run when the
 * typed word IS an emote. Anything else stays a command.
 *
 * Names mirror script 7304's keyword list; an emote added to a newer cache and
 * missing here simply loses its shortcut instead of eating a command.
 */
export const EMOTE_SHORTCUT_SCRIPT_ID = 7304;

const EMOTE_NAMES = new Set(
    (
        "yes nod no shakehead bow angry think wave shrug cheer beckon laugh " +
        "jumpforjoy jump yawn dance jig spin headbang cry blowkiss panic raspberry " +
        "clap salute goblinbow goblinsalute glassbox climbrope lean glasswall idea " +
        "stamp flap slaphead zombiewalk zombiedance scared rabbithop bunnyhop " +
        "skillcape zombiehand musiccape situp pushup pressup starjump jog run flex " +
        "ash hypermobiledrinker uritransform uri smoothdance crazydance " +
        "premiershield explore relicunlock relic party trick bats sit " +
        "fortissalute fortis"
    ).split(" "),
);

/** True when the typed chat line is exactly an emote shortcut, prefix aside. */
export function isEmoteShortcut(typed: string): boolean {
    const word = String(typed ?? "")
        .trim()
        .toLowerCase()
        .replace(/^(::|!)/, "");
    return EMOTE_NAMES.has(word);
}
