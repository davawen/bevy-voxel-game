use bevy::prelude::*;

#[derive(Default, Clone, Copy)]
pub enum Block {
    #[default]
    Air,
    Grass,
    Dirt,
    Stone,
}

impl Block {
    pub fn transparent(&self) -> bool {
        use Block::*;
        match self {
            Air => true,
            _ => false,
        }
    }

    /// Returns wether the given block is a full block
    pub fn full(&self) -> bool {
        use Block::*;
        match self {
            Air => false,
            _ => true,
        }
    }

    pub fn uvs(&self, face: Face) -> Option<[Vec2; 4]> {
        const TEXTURE_BLOCK_SIZE: f32 = 16.0;
        const ATLAS_SIZE: f32 = 256.0;

        use Block::*;
        let atlas_coordinate = match self {
            Grass => match face {
                Face::TOP => Some(IVec2::new(0, 1)),
                face if face.is_side() => Some(IVec2::new(0, 0)),
                _ => Some(IVec2::new(1, 0))
            },
            Dirt => Some(IVec2::new(1, 0)),
            Stone => Some(IVec2::new(2, 0)),
            _ => None,
        };

        atlas_coordinate
            // Get the 2 corners in uv space
            .and_then(|c| {
                Some((
                    c.as_vec2() * TEXTURE_BLOCK_SIZE / ATLAS_SIZE,
                    ((c + 1).as_vec2() * TEXTURE_BLOCK_SIZE - 1.0) / ATLAS_SIZE,
                ))
            })
            // Get the 4 corners
            .and_then(|(uv0, uv1)| {
                Some([uv0, Vec2::new(uv1.x, uv0.y), uv1, Vec2::new(uv0.x, uv1.y)])
            })
            // Rotate according to the face (clockwise order and all that jazz)
            .and_then(|mut uvs| {
                match face {
                    Face::WEST | Face::SOUTH => { uvs.reverse(); Some(uvs) },
                    _ => Some(uvs)
                }
            })
    }
}

#[derive(Clone, Copy)]
pub enum Face {
    TOP,
    BOTTOM,
    EAST,
    WEST,
    NORTH,
    SOUTH,
}

impl Face {
    pub const fn normal(self) -> IVec3 {
        use Face::*;
        match self {
            TOP => IVec3::Y,
            BOTTOM => IVec3::NEG_Y,
            EAST => IVec3::X,
            WEST => IVec3::NEG_X,
            NORTH => IVec3::Z,
            SOUTH => IVec3::NEG_Z,
        }
    }

    pub const fn normal_vec3(self) -> Vec3 {
        use Face::*;
        match self {
            TOP => Vec3::Y,
            BOTTOM => Vec3::NEG_Y,
            EAST => Vec3::X,
            WEST => Vec3::NEG_X,
            NORTH => Vec3::Z,
            SOUTH => Vec3::NEG_Z,
        }
    }

    pub const fn is_any(self) -> bool {
        true
    }

    pub const fn is_vertical(self) -> bool {
        use Face::*;
        matches!(self, TOP | BOTTOM)
    }

    pub const fn is_side(self) -> bool {
        use Face::*;
        matches!(self, EAST | WEST | NORTH | SOUTH)
    }
}
