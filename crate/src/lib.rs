use filmshell::game::{Grenade, Level, LevelBox};
use filmshell::{
    Ammo, Chunk, ChunkKind, Coordinates, Energy, EquipmentCharge, Event, Film, GameMode,
    GameVariant, GrenadeCount, GroundWeapon, Header, Heat, Highlight, HillScore, Input, Kill, Life,
    MedalAward, Player, PlayerId, Ride, Sample, Scoreboard, Tick, TimedEvent, Vehicle, WorldObject,
    Zone,
};
use js_sys::{Array, ArrayBuffer, Reflect, Uint8Array};
use serde::Serialize;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use wasm_bindgen::{prelude::*, JsCast};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FilmData<'a> {
    header: &'a Header,
    clock: FilmClock<'a>,
    players: &'a [Player],
    variant: Option<&'a GameVariant>,
    mode: GameMode,
    level: Option<LevelData<'a>>,
    coordinates: Option<&'a Coordinates>,
    lives: Vec<LifeData<'a>>,
    vehicles: &'a [Vehicle],
    rides: Vec<Ride>,
    world_objects: &'a [WorldObject],
    ground_weapons: &'a [GroundWeapon],
    grenade_flights: Vec<GrenadeFlightData<'a>>,
    zones: &'a [Zone],
    hill_score: Option<&'a HillScore>,
    events: Vec<Cow<'a, TimedEvent>>,
    kills: Vec<KillData<'a>>,
    highlights: &'a [Highlight],
    medals: Vec<MedalData>,
    scoreboard: &'a Scoreboard,
    states: Vec<PlayerStateData<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FilmClock<'a> {
    frame_count: usize,
    frame_ticks: &'a [u32],
    end: filmshell::Tick,
    ticks_per_frame: u32,
    highlight_zero: Option<f64>,
    hitches: Vec<(u32, u32)>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LevelData<'a> {
    id: filmshell::TagId,
    name: &'a str,
    structure: filmshell::TagId,
    play: LevelBoxData,
    index_bits: u32,
    others: Vec<LevelBoxData>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LevelBoxData {
    index: u32,
    min: [f32; 3],
    max: [f32; 3],
    widths: [u32; 3],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlayerStateData<'a> {
    player_id: PlayerId,
    state: InventoryStateData<'a>,
}

#[derive(Serialize)]
struct GrenadeFlightData<'a> {
    player: PlayerId,
    grenade: Option<Grenade>,
    path: &'a [(Tick, filmshell::Position)],
    end: Tick,
    deleted: bool,
}

#[derive(Default, Serialize)]
struct InventoryStateData<'a> {
    inputs: &'a [Input],
    ammo: Vec<Ammo>,
    energy: Vec<Energy>,
    heat: Vec<Heat>,
    grenades: Vec<GrenadeCount>,
    equipment: Vec<EquipmentCharge>,
}

#[derive(Clone, Copy)]
struct PathOwner {
    previous: PlayerId,
    current: PlayerId,
    from: Tick,
    to: Tick,
}

fn path_player(player: PlayerId, tick: Tick, owners: &[PathOwner]) -> PlayerId {
    let matches: BTreeSet<_> = owners
        .iter()
        .filter(|owner| {
            owner.previous == player
                && owner.from.0 <= tick.0.saturating_add(2)
                && owner.to.0.saturating_add(2) >= tick.0
        })
        .map(|owner| owner.current)
        .collect();
    if matches.len() == 1 {
        return *matches.first().unwrap();
    }
    if matches.is_empty() {
        if let Some(first) = owners
            .iter()
            .filter(|owner| owner.previous == player)
            .min_by_key(|owner| owner.from)
        {
            if tick < first.from {
                return first.current;
            }
        }
    }
    player
}

fn inventory_events<'a>(film: &'a Film, owners: &[PathOwner]) -> Vec<Cow<'a, TimedEvent>> {
    film.events()
        .iter()
        .map(|timed| {
            let player = match &timed.event {
                Event::Pickup(value) => value.player,
                Event::Shot(value) => value.player,
                Event::Action(value) => value.player,
                Event::Throw(value) => value.player,
                Event::EquipmentUse(value) => value.player,
                Event::VehicleExit(value) => value.player,
                _ => return Cow::Borrowed(timed),
            };
            let corrected = path_player(player, timed.tick, owners);
            if corrected == player {
                return Cow::Borrowed(timed);
            }
            let mut event = timed.clone();
            match &mut event.event {
                Event::Pickup(value) => value.player = corrected,
                Event::Shot(value) => value.player = corrected,
                Event::Action(value) => value.player = corrected,
                Event::Throw(value) => value.player = corrected,
                Event::EquipmentUse(value) => value.player = corrected,
                Event::VehicleExit(value) => value.player = corrected,
                _ => {}
            }
            Cow::Owned(event)
        })
        .collect()
}

fn inventory_states<'a>(film: &'a Film, owners: &[PathOwner]) -> Vec<PlayerStateData<'a>> {
    let mut states: BTreeMap<PlayerId, InventoryStateData<'a>> = BTreeMap::new();
    for player in film.players() {
        let Some(source) = film.state(player.id) else {
            continue;
        };
        states.entry(player.id).or_default().inputs = &source.inputs;
        for value in &source.ammo {
            states
                .entry(path_player(player.id, value.tick, owners))
                .or_default()
                .ammo
                .push(*value);
        }
        for value in &source.energy {
            states
                .entry(path_player(player.id, value.tick, owners))
                .or_default()
                .energy
                .push(*value);
        }
        for value in &source.heat {
            states
                .entry(path_player(player.id, value.tick, owners))
                .or_default()
                .heat
                .push(*value);
        }
        for value in &source.grenades {
            states
                .entry(path_player(player.id, value.tick, owners))
                .or_default()
                .grenades
                .push(*value);
        }
        for value in &source.equipment {
            states
                .entry(path_player(player.id, value.tick, owners))
                .or_default()
                .equipment
                .push(*value);
        }
    }
    states
        .into_iter()
        .map(|(player_id, mut state)| {
            state.ammo.sort_by_key(|value| value.tick);
            state.energy.sort_by_key(|value| value.tick);
            state.heat.sort_by_key(|value| value.tick);
            state.grenades.sort_by_key(|value| value.tick);
            state.equipment.sort_by_key(|value| value.tick);
            PlayerStateData { player_id, state }
        })
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct KillData<'a> {
    tick: filmshell::Tick,
    kill: &'a Kill,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MedalData {
    tick: Option<filmshell::Tick>,
    award: MedalAward,
}

#[derive(Serialize)]
struct LifeData<'a> {
    player: Option<PlayerId>,
    entity: Option<u16>,
    died: bool,
    death: Option<Tick>,
    samples: &'a [Sample],
}

impl<'a> From<&'a Life> for LifeData<'a> {
    fn from(life: &'a Life) -> Self {
        Self {
            player: life.player,
            entity: life.entity,
            died: life.died,
            death: life.death,
            samples: &life.samples,
        }
    }
}

fn metadata_players(payload: &[u8], players: &BTreeMap<u64, PlayerId>) -> BTreeMap<u8, PlayerId> {
    let mut found: BTreeMap<u64, (u64, usize)> = BTreeMap::new();
    for bit in 5..(payload.len() as u64 * 8).saturating_sub(63) {
        let mut reader = filmshell::format::BitReader::at(payload, bit + 48);
        if reader.read(8) != 9 || reader.read(8) != 0 {
            continue;
        }
        let mut reader = filmshell::format::BitReader::at(payload, bit);
        let mut xuid = 0u64;
        for byte in 0..8 {
            xuid |= (reader.read(8) as u64) << (byte * 8);
        }
        if !players.contains_key(&xuid) {
            continue;
        }
        let entry = found.entry(xuid).or_insert((bit, 0));
        entry.1 += 1;
    }
    let mut indexes: BTreeMap<u8, Vec<PlayerId>> = BTreeMap::new();
    for (xuid, (bit, count)) in found {
        if count < 2 {
            continue;
        }
        let index = filmshell::format::BitReader::at(payload, bit - 5).read(5) as u8;
        indexes.entry(index).or_default().push(players[&xuid]);
    }
    indexes
        .into_iter()
        .filter_map(|(index, owners)| (owners.len() == 1).then_some((index, owners[0])))
        .collect()
}

fn biped_creations(payload: &[u8]) -> Vec<(u16, u8)> {
    let mut found = Vec::new();
    let mut bit = 0;
    let limit = payload.len() as u64 * 8;
    while bit + 72 <= limit {
        let mut header = filmshell::format::BitReader::at(payload, bit);
        if header.read(3) != 1 {
            bit += 1;
            continue;
        }
        let index = header.read(13);
        let generation = header.read(2);
        if header.read(6) != 35 || !(512..768).contains(&index) || generation == 0 {
            bit += 1;
            continue;
        }
        if header.read(1) != 1
            || header.read(8) != 13
            || header.read(1) != 1
            || header.read(32) != 0x1876bda0
            || header.read(1) != 0
        {
            bit += 1;
            continue;
        }
        let player = header.read(5) as u8;
        found.push((((generation - 1) * 256 + index - 512) as u16, player));
        bit += 72;
    }
    found
}

fn body_death(
    start: Tick,
    next: Option<Tick>,
    player: PlayerId,
    deaths: &[(Tick, PlayerId)],
) -> Option<Tick> {
    deaths
        .iter()
        .filter(|(tick, owner)| {
            *owner == player && tick.0 >= start.0 && next.is_none_or(|next| tick.0 < next.0)
        })
        .map(|(tick, _)| *tick)
        .min()
}

fn owner_at_start(
    start: Tick,
    owners: &BTreeMap<u16, Vec<(Tick, PlayerId)>>,
    claimed: &BTreeSet<(u16, Tick)>,
) -> Option<(u16, Tick, PlayerId)> {
    let mut candidates = owners.iter().flat_map(|(&entity, entries)| {
        entries.iter().filter_map(move |&(tick, player)| {
            (tick == start && !claimed.contains(&(entity, tick))).then_some((entity, tick, player))
        })
    });
    let first = candidates.next()?;
    candidates.next().is_none().then_some(first)
}

fn owner_for_life(
    start: Tick,
    end: Tick,
    owners: &BTreeMap<u16, Vec<(Tick, PlayerId)>>,
    claimed: &BTreeSet<(u16, Tick)>,
    deaths: &[(Tick, PlayerId)],
    slack: u32,
) -> Option<(u16, Tick, PlayerId)> {
    let mut candidates = Vec::new();
    for (&entity, entries) in owners {
        for &(spawn, player) in entries {
            if claimed.contains(&(entity, spawn)) || spawn.0 > start.0.saturating_add(slack) {
                continue;
            }
            let next = owners
                .values()
                .flatten()
                .filter(|(tick, owner)| *owner == player && tick.0 > spawn.0)
                .map(|(tick, _)| *tick)
                .min();
            if next.is_some_and(|next| start.0 >= next.0) {
                continue;
            }
            let Some(death) = body_death(spawn, next, player, deaths) else {
                continue;
            };
            if death.0.abs_diff(end.0) <= 15.max(slack) {
                candidates.push((entity, spawn, player));
            }
        }
    }
    if candidates.len() == 1 {
        candidates.first().copied()
    } else {
        None
    }
}

fn authoritative_lives(film: &Film) -> Vec<LifeData<'_>> {
    let raw = film.raw();
    let players: BTreeMap<_, _> = film
        .players()
        .iter()
        .filter_map(|player| player.xuid().map(|xuid| (xuid.0, player.id)))
        .collect();
    let mut rosters = BTreeMap::new();
    for packet in raw.packets().filter(|packet| packet.kind == 8) {
        let roster = metadata_players(packet.payload, &players);
        if !roster.is_empty() {
            rosters.insert(packet.chunk, roster);
        }
    }
    let mut owners: BTreeMap<u16, Vec<(Tick, PlayerId)>> = BTreeMap::new();
    for frame in raw.frames() {
        let Some((_, roster)) = rosters.range(..=frame.chunk).next_back() else {
            continue;
        };
        for (entity, index) in biped_creations(frame.payload()) {
            if let Some(&player) = roster.get(&index) {
                let entries = owners.entry(entity).or_default();
                if entries.last() != Some(&(frame.tick, player)) {
                    entries.push((frame.tick, player));
                }
            }
        }
    }
    let deaths: Vec<_> = film
        .kills()
        .map(|(tick, kill)| (tick, kill.victim))
        .collect();
    let slack = film.clock().ticks_per_frame() * 2;
    let mut assignments: Vec<_> = film
        .lives()
        .iter()
        .map(|life| {
            let entity = life.entity?;
            let first = life.samples.first()?;
            owners
                .get(&entity)
                .and_then(|entries| {
                    entries
                        .iter()
                        .rev()
                        .find(|(tick, _)| tick.0 <= first.tick.0.saturating_add(slack))
                })
                .map(|&(tick, player)| (entity, tick, player))
        })
        .collect();
    let mut claimed: BTreeSet<_> = assignments
        .iter()
        .flatten()
        .map(|&(entity, tick, _)| (entity, tick))
        .collect();
    for (source, assignment) in film.lives().iter().zip(&mut assignments) {
        if source.entity.is_some() || assignment.is_some() {
            continue;
        }
        let Some(first) = source.samples.first() else {
            continue;
        };
        let owner = owner_at_start(first.tick, &owners, &claimed).or_else(|| {
            owner_for_life(
                first.tick,
                source.samples.last()?.tick,
                &owners,
                &claimed,
                &deaths,
                slack,
            )
        });
        if let Some(owner) = owner {
            claimed.insert((owner.0, owner.1));
            *assignment = Some(owner);
        }
    }
    film.lives()
        .iter()
        .zip(assignments)
        .map(|(source, assignment)| {
            let mut life = LifeData::from(source);
            let Some((entity, spawn, player)) = assignment else {
                return life;
            };
            let next = owners
                .values()
                .flatten()
                .filter(|(tick, owner)| *owner == player && tick.0 > spawn.0)
                .map(|(tick, _)| *tick)
                .min();
            life.player = Some(player);
            life.entity = Some(entity);
            life.death = body_death(spawn, next, player, &deaths);
            life.died = source.died || life.death.is_some();
            life
        })
        .collect()
}

impl From<LevelBox> for LevelBoxData {
    fn from(level_box: LevelBox) -> Self {
        Self {
            index: level_box.index,
            min: level_box.min,
            max: level_box.max,
            widths: level_box.widths,
        }
    }
}

impl<'a> From<&'a Level> for LevelData<'a> {
    fn from(level: &'a Level) -> Self {
        Self {
            id: level.id,
            name: level.name,
            structure: level.structure,
            play: level.play.into(),
            index_bits: level.index_bits,
            others: level.others.iter().copied().map(Into::into).collect(),
        }
    }
}

#[wasm_bindgen]
pub fn parse_film(chunks: &JsValue) -> Result<String, JsValue> {
    let chunks = Array::from(chunks)
        .iter()
        .map(|input| {
            let index = read_u32(&input, "index")?;
            let chunk_type = read_u32(&input, "chunkType")?;
            let kind = ChunkKind::from_manifest(chunk_type).ok_or_else(|| {
                JsValue::from_str(&format!("Unknown film chunk type: {chunk_type}"))
            })?;
            let buffer = Reflect::get(&input, &JsValue::from_str("bytes"))?
                .dyn_into::<ArrayBuffer>()
                .map_err(|_| JsValue::from_str("Chunk bytes must be an ArrayBuffer"))?;
            let bytes = Uint8Array::new(&buffer).to_vec();
            Ok(Chunk::new(index, kind, bytes))
        })
        .collect::<Result<Vec<_>, JsValue>>()?;
    let film = Film::from_chunks(chunks).map_err(|error| JsValue::from_str(&error.to_string()))?;
    let clock = film.clock();
    let lives = authoritative_lives(&film);
    let owners: Vec<_> = film
        .lives()
        .iter()
        .zip(&lives)
        .filter_map(|(previous, current)| {
            Some(PathOwner {
                previous: previous.player?,
                current: current.player?,
                from: previous.samples.first()?.tick,
                to: previous.samples.last()?.tick,
            })
        })
        .collect();
    let data = FilmData {
        header: film.header(),
        clock: FilmClock {
            frame_count: clock.frame_count(),
            frame_ticks: clock.frame_ticks(),
            end: clock.end(),
            ticks_per_frame: clock.ticks_per_frame(),
            highlight_zero: clock.highlight_zero(),
            hitches: clock.hitches(),
        },
        players: film.players(),
        variant: film.variant(),
        mode: film.mode(),
        level: film.level().map(Into::into),
        coordinates: film.coordinates(),
        lives,
        vehicles: film.vehicles(),
        rides: film
            .rides()
            .iter()
            .map(|ride| {
                let mut corrected = *ride;
                corrected.player = path_player(ride.player, ride.from, &owners);
                corrected
            })
            .collect(),
        world_objects: film.world_objects(),
        ground_weapons: film.ground_weapons(),
        grenade_flights: film
            .grenade_flights()
            .iter()
            .map(|flight| GrenadeFlightData {
                player: flight.path.first().map_or(flight.player, |(tick, _)| {
                    path_player(flight.player, *tick, &owners)
                }),
                grenade: flight.grenade,
                path: &flight.path,
                end: flight.end,
                deleted: flight.deleted,
            })
            .collect(),
        zones: film.zones(),
        hill_score: film.hill_score(),
        events: inventory_events(&film, &owners),
        kills: film
            .kills()
            .map(|(tick, kill)| KillData { tick, kill })
            .collect(),
        highlights: film.highlights(),
        medals: film
            .medals()
            .map(|(tick, award)| MedalData { tick, award })
            .collect(),
        scoreboard: film.scoreboard(),
        states: inventory_states(&film, &owners),
    };
    serde_json::to_string(&data).map_err(|error| JsValue::from_str(&error.to_string()))
}

fn read_u32(value: &JsValue, field: &str) -> Result<u32, JsValue> {
    let number = Reflect::get(value, &JsValue::from_str(field))?
        .as_f64()
        .ok_or_else(|| JsValue::from_str(&format!("Chunk {field} must be a number")))?;
    if !number.is_finite() || number < 0.0 || number > u32::MAX as f64 || number.fract() != 0.0 {
        return Err(JsValue::from_str(&format!(
            "Chunk {field} must be a non-negative integer"
        )));
    }
    Ok(number as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_follows_the_corrected_body_owner_and_preserves_initial_state() {
        let owners = [
            PathOwner {
                previous: PlayerId(4),
                current: PlayerId(4),
                from: Tick(2079),
                to: Tick(2859),
            },
            PathOwner {
                previous: PlayerId(4),
                current: PlayerId(5),
                from: Tick(3337),
                to: Tick(6531),
            },
            PathOwner {
                previous: PlayerId(4),
                current: PlayerId(4),
                from: Tick(6562),
                to: Tick(9212),
            },
        ];
        assert_eq!(path_player(PlayerId(4), Tick(631), &owners), PlayerId(4));
        assert_eq!(path_player(PlayerId(4), Tick(4694), &owners), PlayerId(5));
        assert_eq!(path_player(PlayerId(4), Tick(5617), &owners), PlayerId(5));
        assert_eq!(path_player(PlayerId(4), Tick(6560), &owners), PlayerId(4));
        assert_eq!(path_player(PlayerId(9), Tick(4694), &owners), PlayerId(9));
    }

    #[test]
    fn ambiguous_inventory_ownership_is_not_reassigned() {
        let owners = [
            PathOwner {
                previous: PlayerId(4),
                current: PlayerId(5),
                from: Tick(100),
                to: Tick(200),
            },
            PathOwner {
                previous: PlayerId(4),
                current: PlayerId(6),
                from: Tick(100),
                to: Tick(200),
            },
        ];
        assert_eq!(path_player(PlayerId(4), Tick(150), &owners), PlayerId(4));
    }

    fn write_bits(bits: &mut Vec<u8>, value: u64, width: u32) {
        for offset in (0..width).rev() {
            bits.push(((value >> offset) & 1) as u8);
        }
    }

    fn bytes(bits: &[u8]) -> Vec<u8> {
        let mut out = vec![0; bits.len().div_ceil(8)];
        for (index, bit) in bits.iter().enumerate() {
            out[index / 8] |= bit << (7 - index % 8);
        }
        out
    }

    #[test]
    fn reads_creation_owner_and_generation_without_byte_alignment() {
        let mut bits = vec![0, 0, 0, 0, 0];
        for (value, width) in [
            (1, 3),
            (597, 13),
            (2, 2),
            (35, 6),
            (1, 1),
            (13, 8),
            (1, 1),
            (0x1876bda0, 32),
            (0, 1),
            (4, 5),
        ] {
            write_bits(&mut bits, value, width);
        }
        assert_eq!(biped_creations(&bytes(&bits)), vec![(341, 4)]);
        bits[5 + 24] = 0;
        assert!(biped_creations(&bytes(&bits)).is_empty());
    }

    #[test]
    fn maps_engine_indices_through_repeated_known_xuids() {
        let xuid = 0x0009000312345678u64;
        let mut bits = Vec::new();
        write_bits(&mut bits, 7, 5);
        for byte in xuid.to_le_bytes() {
            write_bits(&mut bits, byte as u64, 8);
        }
        write_bits(&mut bits, 0, 32);
        for byte in xuid.to_le_bytes() {
            write_bits(&mut bits, byte as u64, 8);
        }
        assert_eq!(
            metadata_players(&bytes(&bits), &BTreeMap::from([(xuid, PlayerId(3))])),
            BTreeMap::from([(7, PlayerId(3))])
        );
        assert!(
            metadata_players(&bytes(&bits[..69]), &BTreeMap::from([(xuid, PlayerId(3))]))
                .is_empty()
        );
    }

    #[test]
    fn recovers_an_unassigned_track_only_from_a_unique_unclaimed_creation() {
        let owners = BTreeMap::from([(14, vec![(Tick(4804), PlayerId(4))])]);
        assert_eq!(
            owner_at_start(Tick(4804), &owners, &BTreeSet::new()),
            Some((14, Tick(4804), PlayerId(4)))
        );
        assert_eq!(owner_at_start(Tick(4805), &owners, &BTreeSet::new()), None);
        assert_eq!(
            owner_at_start(Tick(4804), &owners, &BTreeSet::from([(14, Tick(4804))])),
            None
        );
    }

    #[test]
    fn does_not_guess_between_creations_on_the_same_tick() {
        let owners = BTreeMap::from([
            (14, vec![(Tick(4804), PlayerId(4))]),
            (15, vec![(Tick(4804), PlayerId(3))]),
        ]);
        assert_eq!(owner_at_start(Tick(4804), &owners, &BTreeSet::new()), None);
    }

    #[test]
    fn recovers_a_late_position_track_only_with_a_unique_creation_and_death_interval() {
        let owners = BTreeMap::from([
            (14, vec![(Tick(4804), PlayerId(4))]),
            (15, vec![(Tick(4900), PlayerId(3))]),
        ]);
        let deaths = [(Tick(5500), PlayerId(4)), (Tick(5500), PlayerId(3))];
        assert_eq!(
            owner_for_life(
                Tick(4810),
                Tick(5499),
                &owners,
                &BTreeSet::new(),
                &deaths,
                2
            ),
            Some((14, Tick(4804), PlayerId(4)))
        );
        assert_eq!(
            owner_for_life(
                Tick(5000),
                Tick(5499),
                &owners,
                &BTreeSet::new(),
                &deaths,
                2
            ),
            None
        );
        assert_eq!(
            owner_for_life(
                Tick(4810),
                Tick(5499),
                &owners,
                &BTreeSet::from([(14, Tick(4804))]),
                &deaths,
                2
            ),
            None
        );
    }

    #[test]
    fn body_ownership_separates_simultaneous_deaths() {
        let deaths = [(Tick(29390), PlayerId(3)), (Tick(29390), PlayerId(4))];
        assert_eq!(
            body_death(Tick(25160), Some(Tick(30000)), PlayerId(4), &deaths),
            Some(Tick(29390))
        );
    }

    #[test]
    fn does_not_attach_next_rounds_death_to_a_surviving_body() {
        let deaths = [(Tick(32205), PlayerId(0))];
        assert_eq!(
            body_death(Tick(22109), Some(Tick(25162)), PlayerId(0), &deaths),
            None
        );
        assert_eq!(
            body_death(Tick(25162), None, PlayerId(0), &deaths),
            Some(Tick(32205))
        );
    }
}
