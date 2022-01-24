//! This modules contains several iterators used to choose which image/command
//! combination will be processed next. Ultimately, only one of these iterators
//! will likely be used, but it's nice to have them in a separate file to
//! experiment with.


/// Returns the distance between unsigned integers a and b.
fn u_distance(a: usize, b: usize) -> usize {
    if a > b {
        a - b
    } else {
        b - a
    }
}


/// Iterator generator on a 2D array.
///
/// Given a i_pos, i_min, i_max, j_pos j_min, j_max, produces an iterator which 
/// will yield the elements closest to (i, j) first. The exact order of which
/// element will be given first is unclear because it uses a sort algorithm.
pub struct Closest2D {
    elements: Vec<(usize, usize, usize)>,
}

impl Closest2D {
    pub fn new(i: usize, i_min: usize, i_max: usize, j: usize, j_min: usize, j_max: usize) -> Closest2D {
        
        let mut elements: Vec<(usize, usize, usize)> = (i_min..=i_max)
            .flat_map(|k| (j_min..=j_max)
                 .map(move |l| (k, l, u_distance(i, k) + u_distance(j, l))))
            .collect();

        elements.sort_unstable_by_key(|e| e.2);

        let elements = elements
            .into_iter()
            .rev()
            .collect();

        Self {
            elements,
        }
    }
}

impl Iterator for Closest2D {
    type Item = (usize, usize);
    fn next(&mut self) -> Option<Self::Item> {
        if let Some((i, j, _d)) = self.elements.pop() {
            Some((i, j))
        } else {
            None
        }
    }
}


/// Iterator generator on a 2D array.
///
/// Given a i_pos, i_min, i_max, j_pos j_min, j_max, produces an iterator which 
/// will go through all elements between the specified boundaries by going 
/// "vertically" first (i.e. first iteration is over j)
#[derive(Debug)]
#[allow(dead_code)]
pub struct VFirst2D {
    i_min:   usize,
    j_min:   usize,
    j_len:   usize,
    flat:       usize,
    flat_len:   usize,
    flat_start: usize,
    init:    bool,
}

#[allow(dead_code)]
impl VFirst2D {
    pub fn new(i: usize, i_min: usize, i_max: usize, j: usize, j_min: usize, j_max: usize) -> VFirst2D {
        let i_len = i_max + 1 - i_min;
        let j_len = j_max + 1 - j_min;
        let flat_len = i_len * j_len;
        let flat = (i-i_min) * j_len + (j-j_min);

        Self {
            i_min,
            j_min, 
            j_len,
            flat,
            flat_len,
            flat_start: flat,
            init: true, 
        }
    }

    fn index(&self) -> (usize, usize) {
        let i = self.flat / self.j_len;
        let j = self.flat - i * self.j_len;
        (i + self.i_min, j + self.j_min)
    }
    
    fn increment_wrap(&mut self) {
        self.flat += 1;
        if self.flat >= self.flat_len {
            self.flat = 0;
        }
    }

}

impl Iterator for VFirst2D {
    type Item = (usize, usize);
    fn next(&mut self) -> Option<Self::Item> {
        if self.flat != self.flat_start || self.init {
            self.init = false;
            let res = self.index();
            self.increment_wrap();
            Some(res)
        }else{
            None
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vfirst2d_case_1() {
        let v: Vec<(usize, usize)> = VFirst2D::new(0, 0, 3, 0, 0, 2).collect();
        let truth: Vec<(usize, usize)> = vec![
            (0, 0), 
            (0, 1), 
            (0, 2), 
            (1, 0), 
            (1, 1), 
            (1, 2), 
            (2, 0), 
            (2, 1), 
            (2, 2), 
            (3, 0), 
            (3, 1), 
            (3, 2),
        ];

        //for (i, c) in (0..self.imgs.len()).flat_map(|i| (0..self.cmds.len()).map(move |c| (i, c))){
        println!("{:?}", truth);
        println!("{:?}", v);

        assert_eq!(v, truth);
    }


    #[test]
    fn vfirst2d_case_2() {
        let v: Vec<(usize, usize)> = VFirst2D::new(2, 0, 3, 0, 0, 2).collect();
        let truth: Vec<(usize, usize)> = vec![
            (2, 0), 
            (2, 1), 
            (2, 2), 
            (3, 0), 
            (3, 1), 
            (3, 2),
            (0, 0), 
            (0, 1), 
            (0, 2), 
            (1, 0), 
            (1, 1), 
            (1, 2), 
        ];
        println!("{:?}", truth);
        println!("{:?}", v);

        assert_eq!(v, truth);
    }

    #[test]
    fn vfirst2d_case_3() {
        let v: Vec<(usize, usize)> = VFirst2D::new(0, 0, 3, 2, 0, 2).collect();
        let truth: Vec<(usize, usize)> = vec![
            (0, 2), 
            (1, 0), 
            (1, 1), 
            (1, 2), 
            (2, 0), 
            (2, 1), 
            (2, 2), 
            (3, 0), 
            (3, 1), 
            (3, 2),
            (0, 0), 
            (0, 1), 
        ];
        println!("{:?}", truth);
        println!("{:?}", v);

        assert_eq!(v, truth);
    }

    #[test]
    fn vfirst2d_case_4() {
        let v: Vec<(usize, usize)> = VFirst2D::new(6, 1, 6, 0, 0, 0).collect();
        let truth: Vec<(usize, usize)> = vec![
            (6, 0), 
            (1, 0),
            (2, 0), 
            (3, 0), 
            (4, 0), 
            (5, 0), 
        ];
        println!("{:?}", truth);
        println!("{:?}", v);

        assert_eq!(v, truth);
    }
}
