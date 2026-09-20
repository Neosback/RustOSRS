import assert from "node:assert/strict";

import { PlayerEcs } from "../game/ecs/PlayerEcs";
import { Gender, PlayerAppearance } from "../rs/config/player/PlayerAppearance";

const players = new PlayerEcs();
const index = players.allocatePlayer(1);

assert.equal(players.getHeadIconPk(index), -1);
players.setHeadIconPk(index, -1);
assert.equal(players.getHeadIconPk(index), -1, "the no-skull value must remain -1");

players.setHeadIconPk(index, 0);
assert.equal(players.getHeadIconPk(index), 0, "white skulls use sprite index 0");

const appearance = new PlayerAppearance(
    Gender.MALE,
    [0, 0, 0, 0, 0],
    new Array(7).fill(-1),
    new Array(14).fill(-1),
    { prayer: 2, skull: 1 },
);
players.setAppearance(index, appearance);
assert.equal(players.getHeadIconPk(index), 1, "appearance skull state must be authoritative");

players.setHeadIconPk(index, -1);
assert.equal(appearance.headIcons.skull, -1);
assert.equal(players.getHeadIconPk(index), -1, "clearing a skull must update appearance state");

console.log("player head icon test passed");
